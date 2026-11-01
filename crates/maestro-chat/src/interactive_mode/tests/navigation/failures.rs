//! Fork and clone failures through the real runtime's factory and replacement callback.
use super::super::{Setup, World, actions, isolated_case, start, typed::submit};
use super::{append_user, cancellable, fork_panel, tree::finish};
use maestro_app::{CreateAgentSessionRuntimeFactory, DiagnosticErrorInfo};
use maestro_tui::Component;
use std::{cell::Cell, path::Path, rc::Rc};

/// Builds a runtime whose factory or replacement callback returns a distinct failure.
async fn failing(directory: &Path, callback: bool) -> (World, Rc<Cell<usize>>) {
    let calls = Rc::new(Cell::new(0));
    let seen = calls.clone();
    let create: CreateAgentSessionRuntimeFactory = Rc::new(move |_| {
        seen.set(seen.get() + 1);
        Box::pin(async { Err(DiagnosticErrorInfo::error("ordinary fork failure")) })
    });
    let world = Setup {
        create: (!callback).then_some(create),
        ..Setup::default()
    }
    .build(directory);
    start(&world).await;
    if callback {
        let seen = calls.clone();
        world.host.set_rebind_session(Some(Rc::new(move |_| {
            seen.set(seen.get() + 1);
            Box::pin(async { Err(DiagnosticErrorInfo::error("replacement callback failure")) })
        })));
    }
    append_user(&world, "first");
    append_user(&world, "second");
    world.mode.chat_container.clear();
    (world, calls)
}

/// Failures show their original message once, without replay or a success notice.
fn reported(world: &World, callback: bool, calls: &Cell<usize>) {
    assert_eq!(calls.get(), 1);
    let message = if callback {
        "replacement callback failure"
    } else {
        "ordinary fork failure"
    };
    let rows = maestro_app::strip_ansi(&world.mode.chat_container.render(120).join("\n")).unwrap();
    assert_eq!(rows.matches(message).count(), 1, "{rows}");
    assert!(rows.contains("Error:"), "{rows}");
    assert!(!rows.contains("to new session"), "{rows}");
    assert!(!rows.contains("first"), "failure must not replay: {rows}");
    assert!(fork_panel(world).is_none(), "the failed selector closes");
}

/// Runs both clone failure producers through the installed command.
pub(super) fn clone_failures() {
    for (case, callback) in [("ordinary-error", false), ("callback-error", true)] {
        isolated_case(
            "clone_keeps_leaf_position_and_outcome_order",
            case,
            &[],
            async move |directory| {
                let (world, calls) = failing(directory, callback).await;
                submit(&world.mode, "/clone");
                finish(&world).await;
                reported(&world, callback, &calls);
            },
        );
    }
}

/// Runs veto, empty selected text and both failure producers through the fork selector.
pub(super) fn fork_outcomes() {
    for (case, callback) in [("ordinary-error", false), ("callback-error", true)] {
        isolated_case(
            "fork_selector_owns_focus_selection_and_restore",
            case,
            &[],
            async move |directory| {
                let (world, calls) = failing(directory, callback).await;
                actions::run_action(&world, "app.session.fork");
                world.terminal.send_input("\r");
                finish(&world).await;
                reported(&world, callback, &calls);
                world.terminal.send_input("restored");
                assert_eq!(world.mode.editor.get_text(), "restored");
            },
        );
    }
    isolated_case(
        "fork_selector_owns_focus_selection_and_restore",
        "veto",
        &[],
        async |directory| {
            let world = cancellable(directory).await;
            let selected = append_user(&world, "kept");
            let old = world.host.session();
            world.mode.chat_container.clear();
            world.mode.editor.set_text("draft");
            actions::run_action(&world, "app.session.fork");
            let rows = fork_panel(&world).unwrap().render(80).join("\n");
            assert!(rows.contains("kept"), "{selected}: {rows}");
            world.terminal.send_input("\r");
            finish(&world).await;
            assert!(Rc::ptr_eq(&old, &world.host.session()));
            assert!(world.mode.chat_container.render(80).is_empty());
            assert!(fork_panel(&world).is_none());
            world.terminal.send_input("!");
            assert_eq!(world.mode.editor.get_text(), "draft!");
        },
    );
    isolated_case(
        "fork_selector_owns_focus_selection_and_restore",
        "empty-text",
        &[],
        fork_empty_text,
    );
}

/// A retained selection callback handles an empty user message not listed in the panel.
async fn fork_empty_text(directory: &Path) {
    let world = super::super::started(directory).await;
    append_user(&world, "first");
    let empty = append_user(&world, "");
    actions::run_action(&world, "app.session.fork");
    // Empty messages are not listed; the retained callback still accepts their identity.
    fork_panel(&world)
        .unwrap()
        .get_message_list()
        .on_select()
        .unwrap()(&empty);
    finish(&world).await;
    assert_eq!(super::user_texts(&world), ["first"]);
    assert_eq!(world.mode.editor.get_text(), "");
    assert!(actions::shows(
        &world.mode.chat_container,
        "Forked to new session"
    ));
}
