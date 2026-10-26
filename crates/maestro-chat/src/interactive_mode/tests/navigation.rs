//! Forking from an earlier user message and cloning the current leaf.
mod escape;
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
    let newest = append_user(&world, "second");
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
    let select = list.on_select().expect("the selection callback");
    select(&newest);
    assert!(world.runtime.drain_futures().is_empty());
    assert!(fork_panel(&world).is_none());
    assert_eq!(user_texts(&world), ["first"]);
    assert_eq!(world.mode.editor.get_text(), "second");
    assert!(shows(&world.mode.chat_container, "Forked to new session"));
}

/// Cancelling closes the panel and leaves the session and the transcript as they were.
async fn cancelling_restores_the_editor(directory: &Path) {
    let world = started(directory).await;
    append_user(&world, "only");
    submit(&world.mode, "/fork");
    let panel = fork_panel(&world).expect("the fork panel");
    let cancel = panel
        .get_message_list()
        .on_cancel()
        .expect("the cancellation callback");
    cancel();
    assert!(world.runtime.drain_futures().is_empty());
    assert!(fork_panel(&world).is_none());
    assert_eq!(user_texts(&world), ["only"]);
    assert!(!shows(&world.mode.chat_container, "Forked to new session"));
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
