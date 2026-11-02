//! The double interrupt on an empty editor, navigation keys and navigation commands.
use super::super::{
    Setup, World, isolated, lifecycle::Effects, start, typed::recalled, typed::submit,
};
use super::{append_user, fork_panel, tree::finish, tree::panel};
use crate::TreeSelectorComponent;
use crate::components::session_selector::SessionSelectorComponent;
use maestro_app::DoubleEscapeAction;
use maestro_tui::tui::InputHandler;
use std::{path::Path, rc::Rc};

/// The interrupt key's default sequence.
const ESCAPE: &str = "\x1b";

/// Starts a mode over `effects` with `action` configured and one user message.
async fn escaping(directory: &Path, effects: &Rc<Effects>, action: DoubleEscapeAction) -> World {
    let world = Setup {
        lifecycle: effects.clone(),
        configure: Box::new(move |settings| settings.set_double_escape_action(action)),
        ..Setup::default()
    }
    .build(directory);
    start(&world).await;
    append_user(&world, "only");
    world
}

/// Which navigation view the editor container shows, if any.
fn opened(world: &World) -> Option<&'static str> {
    if panel::<TreeSelectorComponent>(world).is_some() {
        Some("tree")
    } else if fork_panel(world).is_some() {
        Some("fork")
    } else if panel::<SessionSelectorComponent>(world).is_some() {
        Some("resume")
    } else {
        None
    }
}

/// Changes the configured double-interrupt action of the running session.
fn configure(world: &World, action: DoubleEscapeAction) {
    world
        .host
        .session()
        .settings_manager()
        .borrow_mut()
        .set_double_escape_action(action);
}

#[test]
fn double_escape_uses_strict_500ms_and_configured_destination() {
    isolated(
        "double_escape_uses_strict_500ms_and_configured_destination",
        async |directory| {
            for (action, view) in [
                (DoubleEscapeAction::Tree, Some("tree")),
                (DoubleEscapeAction::Fork, Some("fork")),
                (DoubleEscapeAction::None, None),
            ] {
                for delta in [-1, 0, 499, 500, 501] {
                    let effects = Rc::new(Effects::default());
                    let world = escaping(directory, &effects, action.clone()).await;
                    world.mode.editor.handle_input(ESCAPE);
                    assert_eq!(opened(&world), None);
                    effects.now.set(10000 + delta);
                    world.mode.editor.set_text("\u{feff} ");
                    world.mode.editor.handle_input(ESCAPE);
                    let paired = view.is_some() && delta < 500;
                    assert_eq!(
                        opened(&world),
                        view.filter(|_| paired),
                        "{action:?} {delta}"
                    );
                    world.mode.restore_editor();
                    configure(&world, DoubleEscapeAction::Tree);
                    world.mode.editor.handle_input(ESCAPE);
                    let recorded = view.is_some() && !paired;
                    assert_eq!(
                        opened(&world),
                        recorded.then_some("tree"),
                        "only an unpaired press records its time: {action:?} {delta}"
                    );
                }
            }
            let effects = Rc::new(Effects::default());
            let world = escaping(directory, &effects, DoubleEscapeAction::Tree).await;
            world.mode.editor.handle_input(ESCAPE);
            configure(&world, DoubleEscapeAction::Fork);
            world.mode.editor.handle_input(ESCAPE);
            assert_eq!(
                opened(&world),
                Some("fork"),
                "the setting is read per press"
            );
            fork_panel(&world)
                .unwrap()
                .get_message_list()
                .on_cancel()
                .unwrap()();
            world.mode.editor.handle_input(ESCAPE);
            assert_eq!(opened(&world), None, "a completed pair starts over");
            world.mode.editor.handle_input(ESCAPE);
            assert_eq!(opened(&world), Some("fork"), "the restarted pair completes");
            assert!(world.runtime.drain_futures().is_empty());
        },
    );
}

#[test]
fn escape_navigation_respects_prior_interrupt_and_text_branches() {
    isolated(
        "escape_navigation_respects_prior_interrupt_and_text_branches",
        async |directory| {
            for draft in ["", "draft", "\u{85}"] {
                let effects = Rc::new(Effects::default());
                let world = escaping(directory, &effects, DoubleEscapeAction::Tree).await;
                world.mode.editor.set_text(draft);
                world.mode.editor.handle_input(ESCAPE);
                world.mode.editor.handle_input(ESCAPE);
                let view = draft.is_empty().then_some("tree");
                assert_eq!(opened(&world), view, "{draft:?}");
                assert_eq!(world.mode.editor.get_text(), draft);
            }
            let effects = Rc::new(Effects::default());
            let world = escaping(directory, &effects, DoubleEscapeAction::Tree).await;
            world.mode.is_bash_mode.set(true);
            world.mode.editor.handle_input(ESCAPE);
            assert!(!world.mode.is_bash_mode.get(), "shell mode clears first");
            world.mode.editor.handle_input(ESCAPE);
            assert_eq!(
                opened(&world),
                None,
                "the shell branch takes the first press"
            );
            world.mode.editor.handle_input(ESCAPE);
            assert_eq!(
                opened(&world),
                Some("tree"),
                "the empty editor pairs afterwards"
            );
            let world = super::super::queues::held_shell(directory).await;
            append_user(&world, "only");
            let ((), errors) = super::super::driving(&world, async {
                let command = super::super::queues::start_held(&world);
                world.mode.editor.handle_input(ESCAPE);
                world.mode.editor.handle_input(ESCAPE);
                assert_eq!(opened(&world), None, "a running shell is cancelled instead");
                super::super::queues::until_settled(command).await;
            })
            .await;
            assert!(errors.is_empty(), "{errors:?}");
            let effects = Rc::new(Effects::default());
            let world = escaping(directory, &effects, DoubleEscapeAction::Tree).await;
            super::super::streaming(&world, async |world, _| {
                world.mode.editor.handle_input(ESCAPE);
                world.mode.editor.handle_input(ESCAPE);
                assert_eq!(opened(world), None, "streaming restores and aborts instead");
            })
            .await;
        },
    );
}

#[test]
fn navigation_commands_share_the_installed_submission_route() {
    isolated(
        "navigation_commands_share_the_installed_submission_route",
        async |directory| {
            let world = Setup::default().build(directory);
            start(&world).await;
            append_user(&world, "only");
            let mut waiter = super::super::typed::waiting(&world.mode);
            for (command, view) in [
                ("/fork", Some("fork")),
                ("/tree", Some("tree")),
                ("/resume", Some("resume")),
                ("/clone", None),
            ] {
                world.mode.editor.set_text(command);
                submit(&world.mode, command);
                assert_eq!(world.mode.editor.get_text(), "", "{command}");
                assert_eq!(opened(&world), view, "{command}");
                world.mode.restore_editor();
                finish(&world).await;
            }
            assert!(super::super::typed::poll(&mut waiter).is_none());
            let history = recalled(&world.mode.editor);
            assert!(
                !history.iter().any(|text| text.starts_with('/')),
                "{history:?}"
            );
        },
    );
}

/// Writes navigation bindings for the next mode in `directory`.
fn bind(directory: &Path, tree: &str, fork: &str, resume: &str) {
    std::fs::write(
        directory.join("keybindings.json"),
        serde_json::json!({
            "app.session.tree": tree, "app.session.fork": fork, "app.session.resume": resume,
        })
        .to_string(),
    )
    .unwrap();
}

/// Presses `key` on a fresh editor container and reports the opened view.
fn pressed(world: &World, key: &str) -> Option<&'static str> {
    world.mode.restore_editor();
    world.mode.editor.handle_input(key);
    opened(world)
}

#[test]
fn navigation_keys_use_existing_registered_actions() {
    isolated(
        "navigation_keys_use_existing_registered_actions",
        async |directory| {
            bind(directory, "alt+t", "alt+f", "alt+r");
            let world = Setup::default().build(directory);
            start(&world).await;
            append_user(&world, "only");
            assert_eq!(pressed(&world, "\x1bt"), Some("tree"));
            assert_eq!(pressed(&world, "\x1bf"), Some("fork"));
            assert_eq!(pressed(&world, "\x1br"), Some("resume"));
            finish(&world).await;
        },
    );
}

#[test]
fn navigation_keys_follow_live_bindings_and_release_the_mode() {
    isolated(
        "navigation_keys_follow_live_bindings_and_release_the_mode",
        async |directory| {
            for (keys, stale) in [
                (["\x1bt", "\x1bf", "\x1br"], ["\x1bu", "\x1bg", "\x1bs"]),
                (["\x1bu", "\x1bg", "\x1bs"], ["\x1bt", "\x1bf", "\x1br"]),
            ] {
                let names = keys.map(|key| format!("alt+{}", &key[1..]));
                bind(directory, &names[0], &names[1], &names[2]);
                let world = Setup::default().build(directory);
                start(&world).await;
                append_user(&world, "only");
                for (key, view) in keys.iter().zip(["tree", "fork", "resume"]) {
                    assert_eq!(pressed(&world, key), Some(view), "{key:?}");
                }
                for key in stale.iter().chain(&["t", "\x14"]) {
                    let view = pressed(&world, key);
                    assert!(view.is_none(), "{key:?} opened {view:?}");
                }
                finish(&world).await;
                let World { mode, runtime, .. } = world;
                let editor = Rc::clone(&mode.editor);
                let weak = Rc::downgrade(&mode);
                drop(mode);
                assert!(weak.upgrade().is_none(), "bound keys do not keep the mode");
                for key in keys {
                    editor.handle_input(key);
                }
                assert!(runtime.drain_futures().is_empty());
            }
        },
    );
}
