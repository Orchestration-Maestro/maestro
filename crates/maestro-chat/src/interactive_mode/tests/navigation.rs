//! Forking from an earlier user message and cloning the current leaf.
mod escape;
mod failures;
mod flush;
mod replacement;
pub(super) mod resume;
mod resume_options;
mod tree;
mod tree_choices;

use super::actions::shows;

use super::typed::{poll, recalled, submit, waiting};
use super::{Setup, World, isolated_case, started};
use crate::UserMessageSelectorComponent;
use maestro_app::{
    ExtensionFactory, Local, Presence, ReadonlySessionManager, SessionBeforeForkEvent,
    SessionBeforeForkResult,
};
use maestro_tui::Component;
use std::{path::Path, rc::Rc};

/// Appends a user message and returns its entry identity.
pub(super) fn append_user(world: &World, text: &str) -> String {
    world
        .host
        .session()
        .session_manager()
        .borrow_mut()
        .append_message(super::transcript::user_record(text))
        .unwrap()
}

/// The text of every user message the current session holds, in entry order.
fn user_texts(world: &World) -> Vec<String> {
    world
        .host
        .session()
        .get_user_messages_for_forking()
        .into_iter()
        .map(|message| message.text)
        .collect()
}

/// The fork panel the editor container shows, if it shows one.
fn fork_panel(world: &World) -> Option<Rc<UserMessageSelectorComponent>> {
    let panels = super::models::shown(world);
    let component = panels.first()?.clone();
    (component as Rc<dyn std::any::Any>).downcast().ok()
}

/// An extension that cancels every fork.
fn cancelling_forks() -> ExtensionFactory<Local> {
    Rc::new(|api| {
        api.on::<SessionBeforeForkEvent, SessionBeforeForkResult>(
            "session_before_fork".into(),
            Rc::new(|_, _| {
                Box::pin(std::future::ready(Ok(Some(SessionBeforeForkResult {
                    cancel: Presence::Present(true),
                    skip_conversation_restore: Presence::Missing,
                }))))
            }),
        )
        .unwrap();
        Box::pin(std::future::ready(Ok(())))
    })
}

#[test]
fn clone_keeps_leaf_position_and_outcome_order() {
    failures::clone_failures();
    isolated_case(
        "clone_keeps_leaf_position_and_outcome_order",
        "leaf",
        &[],
        cloning_without_a_leaf_reports_it,
    );
    isolated_case(
        "clone_keeps_leaf_position_and_outcome_order",
        "success",
        &[],
        cloning_keeps_the_exact_leaf,
    );
    isolated_case(
        "clone_keeps_leaf_position_and_outcome_order",
        "cancelled",
        &[],
        cancelling_a_clone_keeps_the_session,
    );
}

/// A session without a leaf reports that nothing can be cloned and keeps its session.
async fn cloning_without_a_leaf_reports_it(directory: &Path) {
    let world = started(directory).await;
    let untouched = world.host.session();
    submit(&world.mode, "/clone");
    let errors = world.runtime.drain_futures();
    assert!(errors.is_empty(), "clone failed: {errors:?}");
    assert!(shows(&world.mode.chat_container, "Nothing to clone yet"));
    assert!(Rc::ptr_eq(&untouched, &world.host.session()));
    assert_eq!(world.mode.editor.get_text(), "");
}

/// The replacement session holds the same history with the cloned leaf selected.
async fn cloning_keeps_the_exact_leaf(directory: &Path) {
    let world = started(directory).await;
    append_user(&world, "first");
    let leaf = append_user(&world, "second");
    let old = world.host.session();
    world.mode.editor.set_text("/clone");
    submit(&world.mode, "/clone");
    let errors = world.runtime.drain_futures();
    assert!(errors.is_empty(), "clone failed: {errors:?}");
    let new = world.host.session();
    assert!(!Rc::ptr_eq(&old, &new));
    assert_eq!(user_texts(&world), ["first", "second"]);
    let manager = new.session_manager();
    assert_eq!(
        manager.borrow().get_leaf_id().map(str::to_owned),
        Some(leaf)
    );
    assert!(shows(&world.mode.chat_container, "Cloned to new session"));
    assert_eq!(world.mode.editor.get_text(), "");
}

/// An extension cancelling the fork leaves the session and the transcript as they were.
async fn cancelling_a_clone_keeps_the_session(directory: &Path) {
    let world = cancellable(directory).await;
    append_user(&world, "kept");
    let old = world.host.session();
    world.mode.chat_container.clear();
    submit(&world.mode, "/clone");
    let errors = world.runtime.drain_futures();
    assert!(errors.is_empty(), "clone failed: {errors:?}");
    assert!(Rc::ptr_eq(&old, &world.host.session()));
    assert!(!shows(&world.mode.chat_container, "Cloned to new session"));
    assert_eq!(world.mode.editor.get_text(), "");
}

#[test]
fn fork_selector_owns_focus_selection_and_restore() {
    failures::fork_outcomes();
    isolated_case(
        "fork_selector_owns_focus_selection_and_restore",
        "empty",
        &[],
        async |directory| {
            let world = started(directory).await;
            submit(&world.mode, "/fork");
            assert!(world.runtime.drain_futures().is_empty());
            assert!(shows(
                &world.mode.chat_container,
                "No messages to fork from"
            ));
            assert!(fork_panel(&world).is_none());
            assert_eq!(world.mode.editor.get_text(), "");
        },
    );
    isolated_case(
        "fork_selector_owns_focus_selection_and_restore",
        "selection",
        &[],
        select_forks_before_the_chosen_message,
    );
    isolated_case(
        "fork_selector_owns_focus_selection_and_restore",
        "cancelled",
        &[],
        cancelling_restores_the_editor,
    );
}

/// Selecting the newest message forks before it, so that message is absent from the
/// replacement session and becomes the editor's selected text.
async fn select_forks_before_the_chosen_message(directory: &Path) {
    let world = started(directory).await;
    append_user(&world, "first");
    append_user(&world, "second");
    world.mode.editor.set_text("/fork");
    submit(&world.mode, "/fork");
    assert!(world.runtime.drain_futures().is_empty());
    assert_eq!(world.mode.editor.get_text(), "");
    let panel = fork_panel(&world).expect("the fork panel");
    let list = Rc::clone(panel.get_message_list());
    let panel_rows = maestro_app::strip_ansi(&panel.render(80).concat()).unwrap();
    assert!(panel_rows.contains("Fork from Message"));
    let rows = maestro_app::strip_ansi(&list.render(80).concat()).unwrap();
    assert!(rows.contains("first"));
    assert!(rows.contains("second"));
    assert!(rows.find("first").unwrap() < rows.find("second").unwrap());
    assert!(
        rows.contains("› second"),
        "the newest message is selected: {rows}"
    );
    world.terminal.send_input("\r");
    assert!(world.runtime.drain_futures().is_empty());
    assert!(fork_panel(&world).is_none());
    assert_eq!(user_texts(&world), ["first"]);
    assert_eq!(world.mode.editor.get_text(), "second");
    world.terminal.send_input("!");
    assert_eq!(world.mode.editor.get_text(), "second!");
    assert!(shows(&world.mode.chat_container, "Forked to new session"));
}

/// Cancelling closes the panel and leaves the session and the transcript as they were.
async fn cancelling_restores_the_editor(directory: &Path) {
    let world = started(directory).await;
    append_user(&world, "only");
    world.mode.editor.set_text("retained draft");
    super::actions::run_action(&world, "app.session.fork");
    let panel = fork_panel(&world).expect("the fork panel");
    let select = panel.get_message_list().on_select().unwrap();
    let cancel = panel.get_message_list().on_cancel().unwrap();
    world.terminal.send_input("\x1b");
    assert!(world.runtime.drain_futures().is_empty());
    assert!(fork_panel(&world).is_none());
    assert_eq!(user_texts(&world), ["only"]);
    assert!(!shows(&world.mode.chat_container, "Forked to new session"));
    assert_eq!(world.mode.editor.get_text(), "retained draft");
    world.terminal.send_input("!");
    assert_eq!(world.mode.editor.get_text(), "retained draft!");
    let weak = Rc::downgrade(&world.mode);
    drop(world.mode);
    assert!(weak.upgrade().is_none());
    select("retired");
    cancel();
    assert!(weak.upgrade().is_none());
}

#[test]
fn clone_command_routes_through_the_editor_without_history_or_waiter_delivery() {
    isolated_case(
        "clone_command_routes_through_the_editor_without_history_or_waiter_delivery",
        "child",
        &[],
        async |directory| {
            let world = started(directory).await;
            append_user(&world, "kept");
            let mut waiter = waiting(&world.mode);
            submit(&world.mode, "/clone");
            let errors: Vec<String> = world
                .runtime
                .drain_futures()
                .iter()
                .map(ToString::to_string)
                .collect();
            assert!(errors.is_empty(), "clone failed: {errors:?}");
            assert!(poll(&mut waiter).is_none());
            let history = recalled(&world.mode.editor);
            assert!(!history.iter().any(|text| text == "/clone"), "{history:?}");
            assert!(shows(&world.mode.chat_container, "Cloned to new session"));
            submit(&world.mode, "later");
            assert_eq!(poll(&mut waiter), Some("later".to_owned()));
        },
    );
}

/// Starts a session with the existing fork veto extension.
async fn cancellable(directory: &Path) -> World {
    let world = Setup {
        extension: Some(cancelling_forks()),
        ..Setup::default()
    }
    .build(directory);
    super::start(&world).await;
    world.runtime.settle().unwrap();
    world
}

#[test]
fn fork_selector_preserves_user_order_and_selected_identity() {
    super::isolated(
        "fork_selector_preserves_user_order_and_selected_identity",
        ordered_forks,
    );
}

/// Selects the earlier of two equal user texts through the configured fork key.
async fn ordered_forks(directory: &Path) {
    let selected = Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = selected.clone();
    let extension: ExtensionFactory<Local> = Rc::new(move |api| {
        let seen = seen.clone();
        api.on::<SessionBeforeForkEvent, SessionBeforeForkResult>(
            "session_before_fork".into(),
            Rc::new(move |event, _| {
                seen.borrow_mut()
                    .push((event.entry_id.clone(), event.position));
                Box::pin(async { Ok(None) })
            }),
        )
        .unwrap();
        Box::pin(async { Ok(()) })
    });
    std::fs::write(
        directory.join("keybindings.json"),
        r#"{"app.session.fork":"alt+f"}"#,
    )
    .unwrap();
    let world = Setup {
        extension: Some(extension),
        ..Setup::default()
    }
    .build(directory);
    super::start(&world).await;
    append_user(&world, "first");
    world
        .host
        .session()
        .session_manager()
        .borrow_mut()
        .append_message(super::transcript::assistant(
            &serde_json::json!([{"type":"text","text":"not a user"}]),
            "stop",
            None,
        ))
        .unwrap();
    let middle = append_user(&world, "same");
    append_user(&world, "same");
    world.terminal.send_input("\x1bf");
    let list = fork_panel(&world).unwrap().get_message_list().clone();
    let rows = maestro_app::strip_ansi(&list.render(80).join("\n")).unwrap();
    assert!(rows.find("first").unwrap() < rows.find("same").unwrap());
    assert_eq!(rows.matches("same").count(), 2);
    assert!(!rows.contains("not a user"));
    assert!(rows.contains("› same"));
    world.terminal.send_input("\x1b[A");
    world.terminal.send_input("\r");
    tree::finish(&world).await;
    assert_eq!(
        *selected.borrow(),
        [(middle, maestro_app::ForkPosition::Before)]
    );
    assert_eq!(user_texts(&world), ["first"]);
    assert_eq!(world.mode.editor.get_text(), "same");
    assert!(shows(&world.mode.chat_container, "Forked to new session"));
}
