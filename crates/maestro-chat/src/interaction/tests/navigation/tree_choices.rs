//! Summary choices and retained tree callbacks through the installed command.
use super::super::{Setup, World, actions::shows, isolated, start, typed::submit};
use super::{
    append_user,
    tree::{finish, panel},
};
use crate::{ExtensionEditorComponent, ExtensionSelectorComponent, TreeSelectorComponent};
use maestro_app::{
    ExtensionFactory, Local, Presence, SessionBeforeTreeEvent, SessionBeforeTreeResult,
};
use maestro_tui::Component;
use std::{cell::RefCell, rc::Rc};

/// Summary options observed by the real navigation operation.
type Choices = Rc<RefCell<Vec<(bool, Presence<String>)>>>;

/// Supplies a summary or a veto through the existing extension event.
fn summary_extension(choices: Choices, cancel: bool) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let choices = Rc::clone(&choices);
        api.on::<SessionBeforeTreeEvent, SessionBeforeTreeResult>(
            "session_before_tree".into(),
            Rc::new(move |event, _| {
                choices.borrow_mut().push((
                    event.preparation.user_wants_summary,
                    event.preparation.custom_instructions.clone(),
                ));
                let result = serde_json::from_value(serde_json::json!({
                    "cancel": cancel, "summary": {"summary": "controlled branch summary"}
                }))
                .unwrap();
                Box::pin(async move { Ok(Some(result)) })
            }),
        )
        .unwrap();
        Box::pin(async { Ok(()) })
    })
}

/// Opens the summary choice for the target.
fn open(world: &World, target: &str) {
    submit(&world.mode, "/tree");
    panel::<TreeSelectorComponent>(world)
        .unwrap()
        .get_tree_list()
        .on_select()
        .unwrap()(target);
    assert!(world.runtime.drain_futures().is_empty());
}

/// Chooses the indexed summary option.
pub(super) fn choose(world: &World, index: usize) {
    let choices = panel::<ExtensionSelectorComponent>(world).unwrap();
    let input = choices.input_handler().unwrap();
    for _ in 0..index {
        input.handle_input("\x1b[B");
    }
    input.handle_input("\r");
    assert!(world.runtime.drain_futures().is_empty());
}

/// Checks the accepted options and the settled transcript and progress presentation.
fn completed_choice(world: &World, seen: &Choices, summarize: bool, text: Option<&str>) {
    assert!(world.mode.status_container.render(100).is_empty());
    assert!(
        !world
            .runtime
            .deadlines()
            .contains(&std::time::Duration::from_millis(80))
    );
    assert_eq!(
        *seen.borrow(),
        [(
            summarize,
            text.map_or(Presence::Missing, |text| Presence::Present(text.to_owned()))
        )]
    );
    assert!(shows(
        &world.mode.chat_container,
        "Navigated to selected point"
    ));
    assert!(!shows(&world.mode.chat_container, "tail"));
    if !summarize {
        use maestro_app::ReadonlySessionManager as _;
        assert_eq!(
            world
                .host
                .session()
                .session_manager()
                .borrow()
                .get_leaf_id(),
            None
        );
    }
}

#[test]
fn tree_choices_keep_reopen_loader_and_navigation_branches() {
    isolated(
        "tree_choices_keep_reopen_loader_and_navigation_branches",
        async |directory| {
            for (index, text) in [
                (0, None),
                (1, None),
                (2, Some("custom instruction")),
                (2, Some("")),
            ] {
                let seen = Choices::default();
                let world = Setup {
                    extension: Some(summary_extension(Rc::clone(&seen), false)),
                    ..Setup::default()
                }
                .build(directory);
                start(&world).await;
                let first = append_user(&world, "first");
                append_user(&world, "tail");
                let original = world.mode.editor.on_escape();
                open(&world, &first);
                choose(&world, index);
                if let Some(text) = text {
                    let editor = panel::<ExtensionEditorComponent>(&world).unwrap();
                    assert!(
                        maestro_app::strip_ansi(&editor.render(100).join("\n"))
                            .unwrap()
                            .contains("Custom summarization instructions")
                    );
                    editor.handle_input(text).unwrap();
                    editor.handle_input("\r").unwrap();
                }
                finish(&world).await;
                assert!(super::tree::restored(&world, original.as_ref()));
                completed_choice(&world, &seen, index != 0, text);
            }
        },
    );
}

#[test]
fn tree_cancellation_returns_to_the_same_selection_without_navigating() {
    isolated(
        "tree_cancellation_returns_to_the_same_selection_without_navigating",
        async |directory| {
            let seen = Choices::default();
            let world = Setup {
                extension: Some(summary_extension(Rc::clone(&seen), true)),
                ..Setup::default()
            }
            .build(directory);
            start(&world).await;
            let first = append_user(&world, "first");
            append_user(&world, "tail");
            open(&world, &first);
            choose(&world, 2);
            panel::<ExtensionEditorComponent>(&world)
                .unwrap()
                .handle_input("\x1b")
                .unwrap();
            assert!(world.runtime.drain_futures().is_empty());
            let choices = panel::<ExtensionSelectorComponent>(&world)
                .expect("custom cancellation returns to choices");
            choices.input_handler().unwrap().handle_input("\x1b");
            finish(&world).await;
            let tree =
                panel::<TreeSelectorComponent>(&world).expect("choice cancellation reopens tree");
            assert_eq!(
                tree.get_tree_list().get_selected_node().unwrap().entry()["id"],
                first
            );
            assert!(seen.borrow().is_empty());
            tree.get_tree_list().on_select().unwrap()(&first);
            assert!(world.runtime.drain_futures().is_empty());
            world.mode.compaction_queued_messages.borrow_mut().push(
                super::super::super::queues::CompactionQueuedMessage {
                    text: "retained on veto".into(),
                    mode: maestro_app::messages::UserMessageDelivery::Steer,
                },
            );
            choose(&world, 1);
            finish(&world).await;
            assert_eq!(
                world.mode.compaction_queued_messages.borrow()[0].text,
                "retained on veto"
            );
            assert!(shows(&world.mode.chat_container, "Navigation cancelled"));
            assert!(!shows(
                &world.mode.chat_container,
                "Navigated to selected point"
            ));
            assert!(panel::<TreeSelectorComponent>(&world).is_none());
        },
    );
}

#[test]
fn tree_empty_current_and_cancel_leave_history_untouched() {
    isolated(
        "tree_empty_current_and_cancel_leave_history_untouched",
        async |directory| {
            let world = Setup::default().build(directory);
            start(&world).await;
            submit(&world.mode, "/tree");
            assert!(shows(&world.mode.chat_container, "No entries in session"));
            let leaf = append_user(&world, "kept");
            submit(&world.mode, "/tree");
            panel::<TreeSelectorComponent>(&world)
                .unwrap()
                .handle_input("\x1b")
                .unwrap();
            assert!(panel::<TreeSelectorComponent>(&world).is_none());
            submit(&world.mode, "/tree");
            panel::<TreeSelectorComponent>(&world)
                .unwrap()
                .get_tree_list()
                .on_select()
                .unwrap()(&leaf);
            finish(&world).await;
            assert!(shows(&world.mode.chat_container, "Already at this point"));
            assert_eq!(super::user_texts(&world), ["kept"]);
        },
    );
}

#[test]
fn navigation_selector_lifetime_releases_the_mode_after_completion() {
    isolated(
        "navigation_selector_lifetime_releases_the_mode_after_completion",
        async |directory| {
            for action in ["app.session.tree", "app.session.resume"] {
                let world = Setup::default().build(directory);
                start(&world).await;
                let leaf = append_user(&world, "kept");
                super::super::actions::run_action(&world, action);
                if action == "app.session.resume" {
                    let sessions = panel::<crate::SessionSelectorComponent>(&world).unwrap();
                    finish(&world).await;
                    let select = sessions.get_session_list().on_select().unwrap();
                    sessions.input_handler().unwrap().handle_input("\x1b");
                    finish(&world).await;
                    world.terminal.send_input("restored");
                    assert_eq!(world.mode.editor.get_text(), "restored");
                    let weak = Rc::downgrade(&world.mode);
                    drop(world.mode);
                    assert!(weak.upgrade().is_none());
                    select("retired-session");
                    sessions.input_handler().unwrap().handle_input("\x1b");
                    assert!(world.runtime.drain_futures().is_empty());
                    assert!(weak.upgrade().is_none());
                    continue;
                }
                let tree = panel::<TreeSelectorComponent>(&world).unwrap();
                let select = tree.get_tree_list().on_select().unwrap();
                tree.handle_input("\x1b").unwrap();
                finish(&world).await;
                let weak = Rc::downgrade(&world.mode);
                drop(world.mode);
                assert!(weak.upgrade().is_none());
                select(&leaf);
                tree.handle_input("\x1b").unwrap();
                assert!(weak.upgrade().is_none());
            }
        },
    );
}
