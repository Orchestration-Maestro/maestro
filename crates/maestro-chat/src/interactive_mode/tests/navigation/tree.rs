//! Session-tree navigation through installed commands, keys and retained panels.
use super::super::typed::submit;
use super::super::{Setup, World, isolated, start};
use super::{super::models::shown, append_user};
use crate::TreeSelectorComponent;
use maestro_app::{ReadonlySessionManager, TreeFilterMode};
use maestro_tui::Component;
use std::{any::Any, rc::Rc};

/// The currently displayed panel of the requested type.
pub(super) fn panel<T: Any>(world: &World) -> Option<Rc<T>> {
    shown(world)
        .first()
        .cloned()
        .and_then(|shown| (shown as Rc<dyn Any>).downcast().ok())
}

#[test]
fn tree_passes_current_leaf_filter_reselection_and_labels() {
    isolated(
        "tree_passes_current_leaf_filter_reselection_and_labels",
        async |directory| {
            for (filter, hint) in [
                (TreeFilterMode::Default, ""),
                (TreeFilterMode::NoTools, "[no-tools]"),
                (TreeFilterMode::UserOnly, "[user]"),
                (TreeFilterMode::LabeledOnly, "[labeled]"),
                (TreeFilterMode::All, "[all]"),
            ] {
                let world = Setup {
                    configure: Box::new(move |settings| settings.set_tree_filter_mode(filter)),
                    ..Setup::default()
                }
                .build(directory);
                start(&world).await;
                install_tree_theme(&world);
                let first = append_user(&world, "first");
                let second = append_user(&world, "second");
                submit(&world.mode, "/tree");
                let tree = panel::<TreeSelectorComponent>(&world)
                    .expect("tree command opens the retained tree");
                let rows = maestro_app::strip_ansi(
                    &tree.try_render(80).expect("tree rows render").join("\n"),
                )
                .unwrap();
                assert!(rows.contains(hint));
                if filter == TreeFilterMode::LabeledOnly {
                    assert!(rows.contains("No entries found"));
                    assert!(tree.get_tree_list().get_selected_node().is_none());
                } else {
                    assert!(
                        rows.contains("first") && rows.contains("second"),
                        "filter {filter:?}: {rows}"
                    );
                    assert_eq!(
                        tree.get_tree_list().get_selected_node().unwrap().entry()["id"],
                        second
                    );
                    tree.handle_input("\x1b[A").unwrap();
                    assert_eq!(
                        tree.get_tree_list().get_selected_node().unwrap().entry()["id"],
                        first
                    );
                    assert_labels(&world, &tree, &first);
                }
                tree.handle_input("\x1b").unwrap();
                assert!(panel::<TreeSelectorComponent>(&world).is_none());
                assert!(world.runtime.drain_futures().is_empty());
            }
        },
    );
}

/// Publishes the shipped tree palette, including selected-row backgrounds.
fn install_tree_theme(world: &World) {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../maestro-theme/assets/theme/dark.json"
    );
    world
        .themes
        .set_theme_instance(Rc::new(
            maestro_theme::load_theme_from_path(path, None).unwrap(),
        ))
        .unwrap();
}

/// Setting and removing a selected label persists on the actual session manager.
fn assert_labels(world: &World, tree: &TreeSelectorComponent, first: &str) {
    tree.handle_input("L").unwrap();
    tree.handle_input("label").unwrap();
    tree.handle_input("\r").unwrap();
    let session = world.host.session();
    assert_eq!(
        session.session_manager().borrow().get_label(first),
        Some(&serde_json::json!("label"))
    );
    tree.handle_input("L").unwrap();
    tree.handle_input("\x01").unwrap();
    tree.handle_input("\x0b").unwrap();
    tree.handle_input("\r").unwrap();
    let manager = session.session_manager().borrow();
    assert_eq!(manager.get_label(first), None);
    let entries = manager.get_entries();
    let label = entries.last().unwrap();
    assert_eq!(label["type"], "label");
    assert_eq!(label["targetId"], first);
    assert!(
        label.get("label").is_none(),
        "clearing omits the stored label field: {label}"
    );
}

/// Completes every operation admitted to the terminal host.
pub(super) async fn finish(world: &World) {
    loop {
        let operations = world.runtime.take_futures();
        if operations.is_empty() {
            break;
        }
        for operation in operations {
            operation.await.unwrap();
        }
    }
}

/// One recorded text-restoration input and its resulting editor value.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TextCase {
    /// The live draft at navigation completion.
    draft: String,
    /// The target's restored text, absent for an assistant target.
    returned: Option<String>,
    /// The independently recorded final draft.
    editor: String,
}

#[test]
fn tree_restores_text_only_into_an_ecmascript_blank_editor() {
    isolated(
        "tree_restores_text_only_into_an_ecmascript_blank_editor",
        async |directory| {
            let cases: Vec<TextCase> = serde_json::from_str(include_str!(
                "../../../../tests/fixtures/interactive_navigation.json"
            ))
            .unwrap();
            assert_eq!(cases.len(), 12);
            for case in cases {
                let world = Setup {
                    configure: Box::new(|settings| {
                        settings.set_global_key("branchSummary", "skipPrompt", true);
                    }),
                    ..Setup::default()
                }
                .build(directory);
                start(&world).await;
                let target = match case.returned {
                    Some(text) => append_user(&world, &text),
                    None => world
                        .host
                        .session()
                        .session_manager()
                        .borrow_mut()
                        .append_message(super::super::transcript::assistant(
                            &serde_json::json!([{"type":"text","text":"anchor"}]),
                            "stop",
                            None,
                        ))
                        .unwrap(),
                };
                append_user(&world, "discarded tail");
                submit(&world.mode, "/tree");
                world.mode.editor.set_text(&case.draft);
                panel::<TreeSelectorComponent>(&world)
                    .unwrap()
                    .get_tree_list()
                    .on_select()
                    .unwrap()(&target);
                finish(&world).await;
                assert_eq!(world.mode.editor.get_text(), case.editor);
                assert!(super::super::actions::shows(
                    &world.mode.chat_container,
                    "Navigated to selected point"
                ));
                assert!(!super::super::actions::shows(
                    &world.mode.chat_container,
                    "discarded tail"
                ));
            }
        },
    );
}

/// A scripted summary answer that arrives only after its request is aborted.
struct UntilAborted(std::sync::Arc<tokio::sync::Notify>);

impl UntilAborted {
    /// Waits for the request's cancellation, then answers.
    fn respond<E: maestro_models::ProviderExecution>(
        input: &maestro_models::FauxResponseInput<E>,
        admitted: &tokio::sync::Notify,
    ) -> impl Future<
        Output = Result<maestro_app::AssistantMessage, maestro_app::DiagnosticErrorInfo>,
    > + Send
    + 'static {
        let common = match input.options.as_ref().unwrap() {
            maestro_models::StreamOptionsInput::Raw(options) => &options.common,
            maestro_models::StreamOptionsInput::Simple(options) => &options.common,
        };
        let signal = common.signal.clone().unwrap();
        admitted.notify_one();
        async move {
            signal.cancelled().await;
            Ok(maestro_models::faux_assistant_message(
                "late",
                maestro_models::FauxAssistantMessageOptions::default(),
            ))
        }
    }
}

impl
    maestro_models::Invoke<
        maestro_app::Local,
        maestro_models::FauxResponseInput<maestro_app::Local>,
    > for UntilAborted
{
    type Return = <maestro_app::Local as maestro_models::Execution>::Future<
        'static,
        Result<maestro_app::AssistantMessage, maestro_app::DiagnosticErrorInfo>,
    >;
    fn invoke(&self, input: maestro_models::FauxResponseInput<maestro_app::Local>) -> Self::Return {
        Box::pin(Self::respond(&input, &self.0))
    }
}

impl
    maestro_models::Invoke<
        maestro_models::Threaded,
        maestro_models::FauxResponseInput<maestro_models::Threaded>,
    > for UntilAborted
{
    type Return = <maestro_models::Threaded as maestro_models::Execution>::Future<
        'static,
        Result<maestro_app::AssistantMessage, maestro_app::DiagnosticErrorInfo>,
    >;
    fn invoke(
        &self,
        input: maestro_models::FauxResponseInput<maestro_models::Threaded>,
    ) -> Self::Return {
        Box::pin(Self::respond(&input, &self.0))
    }
}

/// Selects `target` in a freshly opened tree and picks the summary choice `down` rows below
/// the first, leaving the navigation it starts on the host.
fn summarize(world: &World, target: &str, down: usize) {
    submit(&world.mode, "/tree");
    panel::<TreeSelectorComponent>(world)
        .unwrap()
        .get_tree_list()
        .on_select()
        .unwrap()(target);
    assert!(world.runtime.drain_futures().is_empty());
    let choices = panel::<crate::ExtensionSelectorComponent>(world).unwrap();
    let input = choices.input_handler().unwrap();
    for _ in 0..down {
        input.handle_input("\x1b[B");
    }
    input.handle_input("\r");
    assert!(world.runtime.drain_futures().is_empty());
}

/// Whether the editor's interrupt callback is `original`.
pub(super) fn restored(world: &World, original: Option<&Rc<dyn Fn()>>) -> bool {
    match (world.mode.editor.on_escape(), original) {
        (Some(current), Some(original)) => Rc::ptr_eq(&current, original),
        (current, original) => current.is_none() && original.is_none(),
    }
}

/// Aborting a pending provider request stops its loader and restores the exact callback.
async fn cancelled_summary(
    scripted: &super::super::prompts::Scripted,
    first: &str,
    label: &str,
    original: Option<&Rc<dyn Fn()>>,
) {
    let world = &scripted.world;
    let admitted = std::sync::Arc::new(tokio::sync::Notify::new());
    scripted
        .provider
        .set_responses(vec![maestro_models::FauxResponseStep::Factory(
            std::sync::Arc::new(UntilAborted(std::sync::Arc::clone(&admitted))),
        )]);
    summarize(world, first, 1);
    let painted = world.mode.status_container.render(100).concat();
    let theme = world.themes.theme().get().unwrap();
    assert!(painted.contains(&theme.fg(&maestro_theme::ThemeColor::Muted, label).unwrap()));
    assert!(painted.contains(&theme.fg(&maestro_theme::ThemeColor::Accent, "⠋").unwrap()));
    assert!(
        world
            .runtime
            .deadlines()
            .contains(&std::time::Duration::from_millis(80))
    );
    let status = maestro_app::strip_ansi(&painted).unwrap();
    assert!(status.contains(label), "{status}");
    assert!(!restored(world, original));
    admitted.notified().await;
    assert_eq!(scripted.provider.state.call_count(), 1);
    world.mode.editor.on_escape().unwrap()();
    finish(world).await;
    assert!(super::super::actions::shows(
        &world.mode.chat_container,
        "Branch summarization cancelled"
    ));
    let reopened = panel::<TreeSelectorComponent>(world).expect("the tree reopens");
    assert_eq!(
        reopened
            .get_tree_list()
            .get_selected_node()
            .unwrap()
            .entry()["id"],
        first
    );
    assert!(world.mode.status_container.render(100).is_empty());
    assert!(restored(world, original));
    assert!(
        !world
            .runtime
            .deadlines()
            .contains(&std::time::Duration::from_millis(80))
    );
    reopened.handle_input("\x1b").unwrap();
}

#[test]
fn navigation_loader_stops_and_restores_escape_on_every_result() {
    isolated(
        "navigation_loader_stops_and_restores_escape_on_every_result",
        async |directory| {
            let scripted = super::super::prompts::Scripted::new(
                directory,
                crate::InteractiveModeOptions::default(),
            );
            let world = &scripted.world;
            start(world).await;
            let first = append_user(world, "first");
            append_user(world, "second");
            let original = world.mode.editor.on_escape();
            world
                .host
                .services()
                .auth_storage
                .set_runtime_api_key(&scripted.provider.models[0].provider, "controlled-key");
            let label = format!(
                "Summarizing branch... ({} to cancel)",
                maestro_app::key_text("app.interrupt")
            );

            world.mode.compaction_queued_messages.borrow_mut().push(
                super::super::super::queues::CompactionQueuedMessage {
                    text: "retained after summary failure".into(),
                    mode: maestro_app::messages::UserMessageDelivery::FollowUp,
                },
            );
            cancelled_summary(&scripted, &first, &label, original.as_ref()).await;
            assert_eq!(world.mode.compaction_queued_messages.borrow().len(), 1);

            summarize(world, &first, 1);
            finish(world).await;
            assert!(super::super::actions::shows(
                &world.mode.chat_container,
                "No more faux responses queued"
            ));
            assert!(panel::<TreeSelectorComponent>(world).is_none());
            assert!(world.mode.status_container.render(100).is_empty());
            assert!(restored(world, original.as_ref()));
            assert_eq!(
                world.mode.compaction_queued_messages.borrow()[0].text,
                "retained after summary failure"
            );
            assert!(!super::super::actions::shows(
                &world.mode.chat_container,
                "retained after summary failure"
            ));
            scripted.finish();
        },
    );
}
