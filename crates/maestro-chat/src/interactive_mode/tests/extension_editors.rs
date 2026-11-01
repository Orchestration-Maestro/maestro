//! Extension editors replacing the default editor through the retained host.
use super::extension_ui::port;
use super::extension_widgets::with_factories;
use super::{Setup, World, isolated};
use crate::CustomEditor;
use maestro_app::messages::UserMessageDelivery;
use maestro_app::utils::clipboard_image::ClipboardImage;
use maestro_app::{
    ActionHandlers, CustomEditorComponent, EditorFactory, EditorFactoryOutcome, ExtensionAPI,
    ExtensionFactory, ExtensionKeybindings, ExtensionShortcutCallback, Local,
};
use maestro_tui::editor_component::{BorderColor, EditorError, TextCallback};
use maestro_tui::tui::{ComponentHandle, InputHandler};
use maestro_tui::{
    AutocompleteProvider, Cancellation, Component, EditorComponent, EditorOptions, FocusFlag,
    Focusable,
};
use std::{
    cell::{Cell, RefCell},
    path::Path,
    rc::{Rc, Weak},
    sync::Arc,
};

/// An error whose allocation identity survives host forwarding.
#[derive(Debug)]
pub(super) struct Original(pub(super) Arc<()>);
impl std::fmt::Display for Original {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("editor operation failed")
    }
}
impl std::error::Error for Original {}

/// What a test reads from, and asks of, the editors its factories build.
#[derive(Default)]
pub(super) struct Knobs {
    /// The editor calls in order.
    pub(super) calls: RefCell<Vec<String>>,
    /// The fallible operation that fails next, by name.
    pub(super) fail: Cell<Option<&'static str>>,
    /// The identity the failures carry.
    pub(super) error: Arc<()>,
    /// The last editor built, kept weakly.
    built: RefCell<Weak<Probe>>,
    /// A one-shot callback invoked by the padding operation.
    on_padding: RefCell<Option<Box<dyn Fn()>>>,
    /// The completion provider the editor last accepted.
    pub(super) provider: RefCell<Option<Rc<dyn AutocompleteProvider<Signal = Cancellation>>>>,
}

impl Knobs {
    /// The last editor built, while it is alive.
    pub(super) fn editor(&self) -> Rc<Probe> {
        self.built.borrow().upgrade().unwrap()
    }
    /// Whether `error` is this witness's original failure.
    pub(super) fn is_original(&self, error: &(dyn std::error::Error + 'static)) -> bool {
        error
            .downcast_ref::<Original>()
            .is_some_and(|error| Arc::ptr_eq(&error.0, &self.error))
    }
    /// Takes the recorded calls.
    fn take(&self) -> Vec<String> {
        self.calls.take()
    }
}

/// A forwarding editor over the chat editor that records its fallible operations and can
/// fail any of them with the witness's original error.
pub(super) struct Probe {
    /// The forwarded editor.
    pub(super) inner: CustomEditor,
    /// The test's view of this editor.
    knobs: Rc<Knobs>,
}

impl Probe {
    /// Records `name`, failing when the witness selected it.
    fn enter(&self, name: &'static str) -> Result<(), EditorError> {
        self.knobs.calls.borrow_mut().push(name.to_owned());
        if self.knobs.fail.get() == Some(name) {
            return Err(Box::new(Original(self.knobs.error.clone())));
        }
        Ok(())
    }
}

impl Focusable for Probe {
    fn focus_flag(&self) -> &FocusFlag {
        self.inner.focus_flag()
    }
}
impl InputHandler for Probe {
    fn handle_input(&self, data: &str) {
        InputHandler::handle_input(&self.inner, data);
    }
}
impl Component for Probe {
    fn render(&self, width: usize) -> Vec<String> {
        self.inner.render(width)
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
    fn focusable(&self) -> Option<&dyn Focusable> {
        Some(self)
    }
}
impl EditorComponent for Probe {
    type Signal = Cancellation;
    fn get_text(&self) -> String {
        EditorComponent::get_text(&self.inner)
    }
    fn set_text(&self, text: &str) {
        EditorComponent::set_text(&self.inner, text);
    }
    fn on_submit(&self) -> Option<TextCallback> {
        EditorComponent::on_submit(&self.inner)
    }
    fn set_on_submit(&self, callback: Option<TextCallback>) {
        EditorComponent::set_on_submit(&self.inner, callback);
    }
    fn on_change(&self) -> Option<TextCallback> {
        EditorComponent::on_change(&self.inner)
    }
    fn set_on_change(&self, callback: Option<TextCallback>) {
        EditorComponent::set_on_change(&self.inner, callback);
    }
    fn border_color(&self) -> Option<BorderColor> {
        EditorComponent::border_color(&self.inner)
    }
    fn set_border_color(&self, color: BorderColor) -> Option<()> {
        self.knobs.calls.borrow_mut().push("border".into());
        EditorComponent::set_border_color(&self.inner, color)
    }
    fn try_get_text(&self) -> Result<String, EditorError> {
        self.enter("text")?;
        self.inner.try_get_text()
    }
    fn try_set_text(&self, text: &str) -> Result<(), EditorError> {
        self.enter("set_text")?;
        self.inner.try_set_text(text)
    }
    fn try_add_to_history(&self, text: &str) -> Result<Option<()>, EditorError> {
        self.enter("history")?;
        self.inner.try_add_to_history(text)
    }
    fn try_insert_text_at_cursor(&self, text: &str) -> Result<Option<()>, EditorError> {
        self.enter("insert")?;
        self.inner.try_insert_text_at_cursor(text)
    }
    fn try_get_expanded_text(&self) -> Result<Option<String>, EditorError> {
        self.enter("expanded")?;
        self.inner.try_get_expanded_text()
    }
    fn try_set_autocomplete_provider(
        &self,
        provider: Rc<dyn AutocompleteProvider<Signal = Cancellation>>,
    ) -> Result<Option<()>, EditorError> {
        self.enter("provider")?;
        drop(self.knobs.provider.replace(Some(Rc::clone(&provider))));
        self.inner.try_set_autocomplete_provider(provider)
    }
    fn try_set_padding_x(&self, padding: f64) -> Result<Option<()>, EditorError> {
        self.enter("padding")?;
        if let Some(callback) = self.knobs.on_padding.take() {
            callback();
        }
        self.inner.try_set_padding_x(padding)
    }
    fn try_set_autocomplete_max_visible(
        &self,
        max_visible: f64,
    ) -> Result<Option<()>, EditorError> {
        self.enter("height")?;
        self.inner.try_set_autocomplete_max_visible(max_visible)
    }
}
impl CustomEditorComponent for Probe {
    fn action_handlers(&self) -> ActionHandlers {
        self.inner.action_handlers()
    }
    fn on_escape(&self) -> Option<Rc<dyn Fn()>> {
        self.inner.on_escape()
    }
    fn set_on_escape(&self, callback: Option<Rc<dyn Fn()>>) {
        self.inner.set_on_escape(callback);
    }
    fn on_ctrl_d(&self) -> Option<Rc<dyn Fn()>> {
        self.inner.on_ctrl_d()
    }
    fn set_on_ctrl_d(&self, callback: Option<Rc<dyn Fn()>>) {
        self.inner.set_on_ctrl_d(callback);
    }
    fn on_paste_image(&self) -> Option<Rc<dyn Fn()>> {
        self.inner.on_paste_image()
    }
    fn set_on_paste_image(&self, callback: Option<Rc<dyn Fn()>>) {
        self.inner.set_on_paste_image(callback);
    }
    fn on_extension_shortcut(&self) -> Option<ExtensionShortcutCallback> {
        self.inner.on_extension_shortcut()
    }
    fn set_on_extension_shortcut(&self, callback: Option<ExtensionShortcutCallback>) {
        self.inner.set_on_extension_shortcut(callback);
    }
}

/// An editor factory building a probe; `custom` selects the application capability and
/// `own` supplies the probe's own special callbacks before the host sees it.
pub(super) fn probe_factory(
    api: &ExtensionAPI<Local>,
    knobs: &Rc<Knobs>,
    custom: bool,
    own: Option<Rc<dyn Fn()>>,
) -> EditorFactory {
    let knobs = knobs.clone();
    api.ui_callback(move |(tui, theme, keys): (_, _, ExtensionKeybindings), _| {
        knobs.calls.borrow_mut().push("factory".into());
        if knobs.fail.get() == Some("factory") {
            return Err(Box::new(Original(knobs.error.clone())));
        }
        let probe = Rc::new(Probe {
            inner: CustomEditor::new(&tui, theme, keys.manager, EditorOptions::default()),
            knobs: knobs.clone(),
        });
        probe.inner.set_on_escape(own.clone());
        probe.inner.set_on_ctrl_d(own.clone());
        probe.inner.set_on_paste_image(own.clone());
        probe
            .inner
            .set_on_extension_shortcut(own.clone().map(|own| {
                Rc::new(move |data: &str| {
                    own();
                    data == "owned"
                }) as ExtensionShortcutCallback
            }));
        knobs.built.replace(Rc::downgrade(&probe));
        Ok(if custom {
            EditorFactoryOutcome::CustomEditor(probe)
        } else {
            EditorFactoryOutcome::Editor(probe)
        })
    })
}

/// The component the editor container shows.
fn shown(world: &World) -> ComponentHandle {
    world.mode.editor_container.children().borrow()[0].clone()
}

/// Whether two handles name one allocation.
fn same<A: ?Sized, B: ?Sized>(left: &Rc<A>, right: &Rc<B>) -> bool {
    std::ptr::addr_eq(Rc::as_ptr(left), Rc::as_ptr(right))
}

/// A started mode whose padding and completion height differ from the editor defaults,
/// with `extra` application actions on the default editor and the factories `build`
/// returns.
async fn editors<T: 'static>(
    directory: &Path,
    build: impl Fn(&ExtensionAPI<Local>) -> Vec<T> + 'static,
) -> (World, Vec<T>) {
    let stored = Rc::new(RefCell::new(Vec::new()));
    let output = stored.clone();
    let factory: ExtensionFactory<Local> = Rc::new(move |api| {
        output.replace(build(&api));
        Box::pin(std::future::ready(Ok(())))
    });
    let world = Setup {
        extension: Some(factory),
        configure: Box::new(|settings| {
            settings.set_editor_padding_x(2.0);
            settings.set_autocomplete_max_visible(7.0);
        }),
        ..Setup::default()
    }
    .build(directory);
    world.mode.init().await.unwrap();
    let callbacks = stored.take();
    (world, callbacks)
}

#[test]
fn editor_replacement_preserves_settings_and_callbacks() {
    isolated(
        "editor_replacement_preserves_settings_and_callbacks",
        async |directory| {
            let knobs = Rc::new(Knobs::default());
            let own_calls = Rc::new(Cell::new(0));
            let recorded = own_calls.clone();
            let own: Rc<dyn Fn()> = Rc::new(move || recorded.set(recorded.get() + 1));
            let (captured, supplied) = (knobs.clone(), own.clone());
            let (world, factories) = editors(directory, move |api| {
                vec![
                    probe_factory(api, &captured, false, None),
                    probe_factory(api, &captured, true, None),
                    probe_factory(api, &captured, true, Some(supplied.clone())),
                ]
            })
            .await;
            let ui = port(&world.mode);
            let default = world.mode.editor.clone();
            for entries in [32, 64, 128] {
                (0..entries).for_each(|n| default.on_action(format!("test.{n}"), Rc::new(|| {})));
                for (index, factory) in factories.iter().enumerate() {
                    installs_over_the_default(&world, &knobs, factory, index != 0);
                }
            }
            ui.set_editor_component(Some(factories[2].clone())).unwrap();
            let probe = knobs.editor();
            assert!(same(&probe.on_escape().unwrap(), &own));
            probe.on_escape().unwrap()();
            probe.on_ctrl_d().unwrap()();
            probe.on_paste_image().unwrap()();
            assert!(probe.on_extension_shortcut().unwrap()("owned"));
            assert_eq!(
                own_calls.get(),
                4,
                "all supplied special callbacks survive installation"
            );
            ui.set_editor_component(Some(factories[1].clone())).unwrap();
            let probe = knobs.editor();
            let calls = Rc::new(Cell::new(0));
            let counted = calls.clone();
            default.set_on_ctrl_d(Some(Rc::new(move || counted.set(counted.get() + 1))));
            probe.on_ctrl_d().unwrap()();
            assert_eq!(calls.get(), 1);
        },
    );
}

/// Installs `factory` over the default editor's draft and checks what the new editor
/// received, then returns its changed text to the default editor.
fn installs_over_the_default(world: &World, knobs: &Knobs, factory: &EditorFactory, copied: bool) {
    let (ui, default) = (port(&world.mode), world.mode.editor.clone());
    default.set_text("draft");
    knobs.take();
    ui.set_editor_component(Some(factory.clone())).unwrap();
    let probe = knobs.editor();
    assert!(same(&shown(world), &probe));
    assert!(probe.inner.focus_flag().get());
    assert_eq!(probe.inner.get_text(), "draft");
    assert!(same(
        &probe.on_submit().unwrap(),
        &default.on_submit().unwrap()
    ));
    assert!(same(
        &probe.on_change().unwrap(),
        &default.on_change().unwrap()
    ));
    assert!(same(
        &EditorComponent::border_color(&probe.inner).unwrap(),
        &EditorComponent::border_color(&*default).unwrap()
    ));
    assert_eq!(probe.inner.get_padding_x().to_bits(), 2.0_f64.to_bits());
    assert_eq!(
        knobs.take(),
        ["factory", "set_text", "border", "padding", "provider"]
    );
    let (custom, defaults) = (probe.inner.action_handlers(), default.action_handlers());
    assert_eq!(custom.borrow().is_empty(), !copied);
    if copied {
        let (custom, defaults) = (custom.borrow(), defaults.borrow());
        assert!(custom.keys().eq(defaults.keys()));
        assert!(
            defaults
                .iter()
                .all(|(name, handler)| Rc::ptr_eq(handler, &custom[name])
                    && Rc::strong_count(handler) == 2)
        );
    }
    probe.inner.set_text("changed");
    ui.set_editor_component(None).unwrap();
    assert_eq!(default.get_text(), "changed");
    assert!(same(&shown(world), &default));
}

#[test]
fn editor_setup_failure_preserves_original_phase_and_ownership() {
    isolated(
        "editor_setup_failure_preserves_original_phase_and_ownership",
        async |directory| {
            let knobs = Rc::new(Knobs::default());
            let captured = knobs.clone();
            let (world, factories) = editors(directory, move |api| {
                vec![
                    probe_factory(api, &captured, true, None),
                    probe_factory(api, &captured, true, None),
                ]
            })
            .await;
            let ui = port(&world.mode);
            let default: ComponentHandle = world.mode.editor.clone();
            for (phase, calls) in [
                ("factory", vec!["factory"]),
                ("set_text", vec!["factory", "set_text"]),
                ("padding", vec!["factory", "set_text", "border", "padding"]),
                (
                    "provider",
                    vec!["factory", "set_text", "border", "padding", "provider"],
                ),
            ] {
                knobs.take();
                knobs.fail.set(Some(phase));
                let error = ui
                    .set_editor_component(Some(factories[0].clone()))
                    .unwrap_err();
                knobs.fail.set(None);
                assert!(knobs.is_original(error.as_ref()), "{phase}");
                assert_eq!(knobs.take(), calls);
                assert!(
                    ui.get_editor_component()
                        .unwrap()
                        .unwrap()
                        .ptr_eq(&factories[0])
                );
                assert!(world.mode.editor_container.children().borrow().is_empty());
                assert!(same(&world.mode.current_editor(), &default));
            }
            ui.set_editor_component(Some(factories[1].clone())).unwrap();
            let installed = knobs.editor();
            knobs.take();
            knobs.fail.set(Some("text"));
            let error = ui
                .set_editor_component(Some(factories[0].clone()))
                .unwrap_err();
            assert!(knobs.is_original(error.as_ref()));
            assert_eq!(knobs.take(), ["text"]);
            assert!(same(&world.mode.current_editor(), &installed));
            assert!(same(&shown(&world), &installed));
        },
    );
}

/// An editor with only the required editor operations.
#[derive(Default)]
struct Bare {
    /// The text.
    text: RefCell<String>,
    /// The submission callback.
    submit: RefCell<Option<TextCallback>>,
    /// The change callback.
    change: RefCell<Option<TextCallback>>,
}
impl InputHandler for Bare {
    fn handle_input(&self, data: &str) {
        self.text.borrow_mut().push_str(data);
    }
}
impl Component for Bare {
    fn render(&self, _width: usize) -> Vec<String> {
        vec![self.text.borrow().clone()]
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
}
impl EditorComponent for Bare {
    type Signal = Cancellation;
    fn get_text(&self) -> String {
        self.text.borrow().clone()
    }
    fn set_text(&self, text: &str) {
        text.clone_into(&mut self.text.borrow_mut());
    }
    fn on_submit(&self) -> Option<TextCallback> {
        self.submit.borrow().clone()
    }
    fn set_on_submit(&self, callback: Option<TextCallback>) {
        drop(self.submit.replace(callback));
    }
    fn on_change(&self) -> Option<TextCallback> {
        self.change.borrow().clone()
    }
    fn set_on_change(&self, callback: Option<TextCallback>) {
        drop(self.change.replace(callback));
    }
}

#[test]
fn plain_editor_optional_controls_are_not_required() {
    isolated(
        "plain_editor_optional_controls_are_not_required",
        async |directory| {
            let built = Rc::new(RefCell::new(Weak::<Bare>::new()));
            let kept = built.clone();
            let (world, factories) = with_factories(directory, move |api| {
                let kept = kept.clone();
                let factory: EditorFactory = api.ui_callback(move |_, _| {
                    let bare = Rc::new(Bare::default());
                    kept.replace(Rc::downgrade(&bare));
                    Ok(EditorFactoryOutcome::Editor(bare))
                });
                vec![factory]
            })
            .await;
            let ui = port(&world.mode);
            world.mode.editor.set_text("draft");
            ui.set_editor_component(factories.into_iter().next())
                .unwrap();
            let bare = built.borrow().upgrade().unwrap();
            assert_eq!(bare.get_text(), "draft");
            assert!(same(&shown(&world), &bare));
            ui.set_editor_text("typed".into()).unwrap();
            assert_eq!(ui.get_editor_text().unwrap(), "typed");
            world.mode.apply_runtime_settings().unwrap();
            world.mode.setup_autocomplete_provider().unwrap();
            world.mode.update_editor_border_color().unwrap();
            assert!(same(
                &bare.on_submit().unwrap(),
                &world.mode.editor.on_submit().unwrap()
            ));
            ui.set_editor_component(None).unwrap();
            assert_eq!(world.mode.editor.get_text(), "typed");
            drop(bare);
            assert!(built.borrow().upgrade().is_none());
        },
    );
}

/// Checks that the editor's missing special callbacks reach the default editor's current
/// ones and that a copied action still acts on the mode.
fn forwards_missing_callbacks(world: &World, probe: &Probe) {
    let default = world.mode.editor.clone();
    let seen = Rc::new(RefCell::new(Vec::<&str>::new()));
    let record = |name: &'static str| -> Rc<dyn Fn()> {
        let seen = seen.clone();
        Rc::new(move || seen.borrow_mut().push(name))
    };
    default.set_on_escape(Some(record("escape")));
    default.set_on_ctrl_d(Some(record("exit")));
    default.set_on_paste_image(Some(record("image")));
    let shortcut = seen.clone();
    default.set_on_extension_shortcut(Some(Rc::new(move |data| {
        shortcut.borrow_mut().push("shortcut");
        data == "x"
    })));
    probe.on_escape().unwrap()();
    probe.on_ctrl_d().unwrap()();
    probe.on_paste_image().unwrap()();
    assert!(probe.on_extension_shortcut().unwrap()("x"));
    assert_eq!(*seen.borrow(), ["escape", "exit", "image", "shortcut"]);
    let expanded = world.mode.tool_output_expanded.get();
    let toggle = probe.inner.action_handlers().borrow()["app.tools.expand"].clone();
    toggle();
    assert_eq!(world.mode.tool_output_expanded.get(), !expanded);
}

/// The failure the mode's consumer of the `failing` editor operation returns or reports.
async fn consumer_failure(world: &World, failing: &str) -> Box<dyn std::error::Error> {
    match failing {
        "expanded" => port(&world.mode).get_editor_text().map(drop).unwrap_err(),
        "provider" => world.mode.setup_autocomplete_provider().unwrap_err(),
        "padding" | "height" => world.mode.apply_runtime_settings().unwrap_err(),
        "history" => world
            .mode
            .queue_compaction_message("entry".into(), UserMessageDelivery::Steer)
            .unwrap_err(),
        _ => {
            drop(world.runtime.take_futures());
            world.mode.paste_clipboard_result(Ok(Some(ClipboardImage {
                bytes: vec![1],
                mime_type: "image/png".into(),
            })));
            let mut reported = world.runtime.take_futures();
            assert_eq!(reported.len(), 1);
            reported.pop().unwrap().await.unwrap_err()
        }
    }
}

/// Checks that runtime settings and the settings panel reach the default editor first and
/// the current editor once, in both directions, and that a panel failure is reported.
async fn settings_reach_both_editors(world: &World, knobs: &Knobs, probe: &Probe) {
    let default = world.mode.editor.clone();
    for (padding, height, visible) in [(3.0, 9.0, 9), (1.0, 4.0, 4)] {
        {
            let session = world.mode.runtime_host.session();
            let mut settings = session.settings_manager().borrow_mut();
            settings.set_editor_padding_x(padding);
            settings.set_autocomplete_max_visible(height);
        }
        world.mode.apply_runtime_settings().unwrap();
        assert_eq!(knobs.take(), ["padding", "height"]);
        assert_eq!(default.get_padding_x().to_bits(), f64::to_bits(padding));
        assert_eq!(probe.inner.get_padding_x().to_bits(), f64::to_bits(padding));
        assert_eq!(probe.inner.get_autocomplete_max_visible(), visible);
        assert_eq!(default.get_autocomplete_max_visible(), visible);
    }
    let panel = world.mode.settings_callbacks(Rc::new(|| {}));
    (panel.on_editor_padding_x_change)(5.0);
    (panel.on_autocomplete_max_visible_change)(6.0);
    (panel.on_enable_skill_commands_change)(true);
    assert_eq!(knobs.take(), ["padding", "height", "provider"]);
    assert_eq!(default.get_padding_x().to_bits(), 5.0_f64.to_bits());
    assert_eq!(probe.inner.get_autocomplete_max_visible(), 6);
    assert!(world.runtime.take_futures().is_empty());
    knobs.fail.set(Some("padding"));
    (panel.on_editor_padding_x_change)(4.0);
    knobs.fail.set(None);
    assert_eq!(default.get_padding_x().to_bits(), 4.0_f64.to_bits());
    let mut reported = world.runtime.take_futures();
    assert!(knobs.is_original(reported.pop().unwrap().await.unwrap_err().as_ref()));
    knobs.take();
}

#[test]
fn custom_editor_factory_retains_optional_application_capabilities() {
    isolated(
        "custom_editor_factory_retains_optional_application_capabilities",
        async |directory| {
            let knobs = Rc::new(Knobs::default());
            let captured = knobs.clone();
            let (world, factories) = with_factories(directory, move |api| {
                vec![
                    probe_factory(api, &captured, true, None),
                    probe_factory(api, &captured, false, None),
                ]
            })
            .await;
            let ui = port(&world.mode);
            ui.set_editor_component(Some(factories[0].clone())).unwrap();
            let probe = knobs.editor();
            let map = probe.inner.action_handlers();
            assert!(Rc::ptr_eq(
                &map,
                &CustomEditorComponent::action_handlers(&*probe)
            ));
            forwards_missing_callbacks(&world, &probe);
            for failing in [
                "expanded", "history", "insert", "provider", "padding", "height",
            ] {
                knobs.fail.set(Some(failing));
                let error = consumer_failure(&world, failing).await;
                assert!(knobs.is_original(error.as_ref()), "{failing}");
            }
            knobs.fail.set(Some("text"));
            let error = ui.set_editor_component(None).unwrap_err();
            assert!(knobs.is_original(error.as_ref()));
            knobs.fail.set(None);
            ui.set_editor_component(Some(factories[1].clone())).unwrap();
            assert!(knobs.editor().inner.action_handlers().borrow().is_empty());
            ui.set_editor_component(None).unwrap();
            let weak = Rc::downgrade(&probe);
            drop((probe, map, factories));
            assert!(weak.upgrade().is_none());
        },
    );
}

/// Returns the exact failure from the selected current-editor text consumer.
async fn text_call_failure(world: &World, operation: &str) -> Box<dyn std::error::Error> {
    match operation {
        "clear" => {
            world.mode.clear_editor();
            let mut reported = world.runtime.take_futures();
            assert_eq!(reported.len(), 1, "the original write failure is reported");
            reported.pop().unwrap().await.unwrap_err()
        }
        "follow-up" => world.mode.handle_follow_up().unwrap_err(),
        "queue" => world
            .mode
            .queue_compaction_message("queued".into(), UserMessageDelivery::Steer)
            .unwrap_err(),
        _ => world
            .mode
            .restore_queued_messages_to_editor(
                super::super::queues::RestoreQueuedMessagesOptions::default(),
            )
            .unwrap_err(),
    }
}

#[test]
fn session_commands_clear_the_current_editor_before_admission() {
    isolated(
        "session_commands_clear_the_current_editor_before_admission",
        async |directory| {
            for (command, fails) in [
                ("/new", false),
                ("/new", true),
                ("/compact", false),
                ("/compact", true),
            ] {
                let knobs = Rc::new(Knobs::default());
                let captured = knobs.clone();
                let (world, factories) = editors(directory, move |api| {
                    vec![probe_factory(api, &captured, true, None)]
                })
                .await;
                port(&world.mode)
                    .set_editor_component(factories.into_iter().next())
                    .unwrap();
                let probe = knobs.editor();
                world.mode.editor.set_text("default draft");
                probe.inner.set_text("current draft");
                knobs.fail.set(fails.then_some("set_text"));
                let session = world.host.session();
                probe.on_submit().unwrap()(command);
                assert_eq!(
                    world.mode.editor.get_text(),
                    "default draft",
                    "the inactive editor is not cleared"
                );
                assert_eq!(
                    probe.inner.get_text(),
                    if fails { "current draft" } else { "" }
                );
                let mut work = world.runtime.take_futures();
                assert_eq!(
                    work.len(),
                    1,
                    "one command or one original error is admitted"
                );
                let result = work.pop().unwrap().await;
                if fails {
                    assert!(knobs.is_original(result.unwrap_err().as_ref()));
                    assert!(Rc::ptr_eq(&session, &world.host.session()));
                } else {
                    result.unwrap();
                    assert_eq!(
                        Rc::ptr_eq(&session, &world.host.session()),
                        command != "/new"
                    );
                }
            }
        },
    );
}

#[test]
fn current_editor_text_calls_preserve_original_failures() {
    isolated(
        "current_editor_text_calls_preserve_original_failures",
        async |directory| {
            for operation in [
                "clear",
                "follow-up",
                "queue",
                "restore-read",
                "restore-write",
            ] {
                let knobs = Rc::new(Knobs::default());
                let captured = knobs.clone();
                let (world, factories) = editors(directory, move |api| {
                    vec![probe_factory(api, &captured, true, None)]
                })
                .await;
                port(&world.mode)
                    .set_editor_component(factories.into_iter().next())
                    .unwrap();
                let probe = knobs.editor();
                if operation.starts_with("restore-") {
                    world
                        .mode
                        .queue_compaction_message("queued".into(), UserMessageDelivery::Steer)
                        .unwrap();
                }
                probe.inner.set_text("draft");
                let submitted = Rc::new(Cell::new(false));
                let seen = submitted.clone();
                probe.set_on_submit(Some(Rc::new(move |_| seen.set(true))));
                knobs.fail.set(Some(if operation == "restore-read" {
                    "text"
                } else {
                    "set_text"
                }));
                world.runtime.settle().unwrap();
                let error = text_call_failure(&world, operation).await;
                assert!(knobs.is_original(error.as_ref()), "{operation}");
                assert_eq!(probe.inner.get_text(), "draft", "{operation}");
                assert!(
                    !submitted.get(),
                    "submission does not follow a failed clear"
                );
                if operation == "clear" {
                    assert_eq!(
                        world.runtime.pending(),
                        0,
                        "no redraw follows a failed clear"
                    );
                }
            }
        },
    );
}

#[test]
fn runtime_settings_follow_editor_replacement_between_callable_phases() {
    isolated(
        "runtime_settings_follow_editor_replacement_between_callable_phases",
        async |directory| {
            for replace_with_custom in [true, false] {
                let (first, second) = (Rc::new(Knobs::default()), Rc::new(Knobs::default()));
                let (a, b) = (first.clone(), second.clone());
                let (world, factories) = editors(directory, move |api| {
                    vec![
                        probe_factory(api, &a, true, None),
                        probe_factory(api, &b, true, None),
                    ]
                })
                .await;
                let ui = port(&world.mode);
                ui.set_editor_component(Some(factories[0].clone())).unwrap();
                let next = replace_with_custom.then(|| factories[1].clone());
                let context = ui.clone();
                first.on_padding.replace(Some(Box::new(move || {
                    context.set_editor_component(next.clone()).unwrap();
                })));
                first.take();
                second.take();
                let session = world.host.session();
                session
                    .settings_manager()
                    .borrow_mut()
                    .set_autocomplete_max_visible(9.0);
                world.mode.apply_runtime_settings().unwrap();
                assert!(
                    !first.take().iter().any(|call| call == "height"),
                    "the retired editor gets no later phase"
                );
                assert_eq!(world.mode.editor.get_autocomplete_max_visible(), 9);
                if replace_with_custom {
                    assert_eq!(second.editor().inner.get_autocomplete_max_visible(), 9);
                    assert_eq!(
                        second
                            .take()
                            .iter()
                            .filter(|call| *call == "height")
                            .count(),
                        1
                    );
                } else {
                    assert!(!world.mode.custom_editor_current());
                    assert!(second.take().is_empty());
                }
            }
        },
    );
}

#[test]
fn custom_editor_controls_use_active_editor_without_crossing_policies() {
    isolated(
        "custom_editor_controls_use_active_editor_without_crossing_policies",
        async |directory| {
            let knobs = Rc::new(Knobs::default());
            let captured = knobs.clone();
            let (world, factories) = editors(directory, move |api| {
                vec![probe_factory(api, &captured, true, None)]
            })
            .await;
            let ui = port(&world.mode);
            let default = world.mode.editor.clone();
            world.mode.apply_runtime_settings().unwrap();
            world.mode.setup_autocomplete_provider().unwrap();
            ui.set_editor_component(factories.into_iter().next())
                .unwrap();
            let probe = knobs.editor();
            knobs.take();
            ui.paste_to_editor("pasted".into()).unwrap();
            assert_eq!(probe.inner.get_text(), "pasted");
            assert_eq!(default.get_text(), "");
            assert_eq!(ui.get_editor_text().unwrap(), "pasted");
            world.mode.update_editor_border_color().unwrap();
            assert_eq!(knobs.take(), ["expanded", "border"]);
            world.mode.setup_autocomplete_provider().unwrap();
            assert_eq!(knobs.take(), ["provider"]);
            settings_reach_both_editors(&world, &knobs, &probe).await;
            world.mode.rebind_current_session().await.unwrap();
            let calls = knobs.take();
            assert_eq!(calls.iter().filter(|call| *call == "provider").count(), 1);
            assert!(calls.starts_with(&["padding".into(), "height".into()]));
            probe.inner.set_text("follow");
            world.mode.handle_follow_up().unwrap();
            assert_eq!(probe.inner.get_text(), "");
            assert_eq!(knobs.take()[0], "expanded");
            assert!(
                probe
                    .inner
                    .action_handlers()
                    .borrow()
                    .contains_key("app.tools.expand")
            );
        },
    );
}
