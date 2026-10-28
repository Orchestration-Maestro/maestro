//! Fresh instances consume pending playback once through ordinary session events.
use super::{Demo, loaded, loaded_on};
use maestro_extensions::{
    ExtensionCommandContextActions, SessionStartEvent, types::SessionShutdownEvent,
};
use maestro_session::session_manager::ReadonlySessionManager;
use std::{cell::Cell, path::Path, rc::Rc};

/// Invokes the original command, preserving its native command context.
async fn command(demo: &Demo, wad: &Path) {
    (demo
        .runner
        .get_command("doom-overlay")
        .unwrap()
        .command
        .handler)(
        wad.to_str().unwrap().into(),
        demo.runner.create_command_context(),
    )
    .await
    .unwrap();
}

/// Starts or resumes the session using the same event the application dispatches.
async fn start(demo: &Demo, reason: &str) {
    let mut event: SessionStartEvent =
        serde_json::from_value(serde_json::json!({"reason": reason})).unwrap();
    demo.runner.emit(&mut event).await.unwrap();
}

/// Stops each independently owned instance and settles its announced tails.
async fn shutdown(demo: &Demo) {
    let mut event: SessionShutdownEvent =
        serde_json::from_value(serde_json::json!({"reason":"quit"})).unwrap();
    demo.runner.emit(&mut event).await.unwrap();
    for tail in demo.tails.take() {
        tail.await.unwrap();
    }
}

/// WAD replacement records a hidden request and a matching one-time consumption.
pub(super) async fn switch(root: &Path) {
    let first = loaded(root, "switch").await;
    let a = root.join("a.wad");
    let b = root.join("b.wad");
    std::fs::write(&a, b"first WAD").unwrap();
    std::fs::write(&b, b"second WAD").unwrap();
    command(&first, &a).await;
    assert_eq!(first.ui.wad.take(), Some(b"first WAD".to_vec()));
    let reloads = Rc::new(Cell::new(0));
    let called = reloads.clone();
    bind_reload(&first, called);
    command(&first, &b).await;
    assert_eq!(
        reloads.get(),
        1,
        "a WAD switch requires a fresh composed instance"
    );
    assert!(
        first.ui.wad.borrow().is_none(),
        "the old instance cannot open the new WAD"
    );
    consumed(root, &first).await;
    shutdown(&first).await;
}

/// Retains session history while replacing every component-owned object.
async fn replacement(root: &Path, original: &Demo) -> Demo {
    let session = maestro_extensions::runner::ExtensionRunnerSession {
        cwd: original.session.cwd.clone(),
        session_manager: original.session.session_manager.clone(),
        model_registry: original.session.model_registry.clone(),
    };
    loaded_on(root, "switch", session, original.history.clone()).await
}

/// Binds the real command import to a counted application reload.
fn bind_reload(first: &Demo, called: Rc<Cell<u32>>) {
    first
        .runner
        .bind_command_context(Some(ExtensionCommandContextActions::new(
            maestro_extensions::CommandContextOperations {
                reload: Rc::new(move || {
                    called.set(called.get() + 1);
                    Box::pin(async { Ok(()) })
                }),
                ..crate::context_actions::silent_command_operations()
            },
        )));
}

/// The stored marker names the exact request it consumed.
fn matching_marker(demo: &Demo) {
    let history = demo.history.borrow();
    let entries = history.get_entries();
    let requested = entries
        .iter()
        .rev()
        .find(|entry| entry["data"]["pending"] == true)
        .unwrap();
    assert!(
        entries
            .iter()
            .any(|entry| entry["data"]["consumed"] == requested["id"])
    );
}

/// A persisted consumption marker survives later events and fresh instances.
async fn consumed(root: &Path, first: &Demo) {
    let fresh = replacement(root, first).await;
    start(&fresh, "reload").await;
    assert_eq!(fresh.ui.wad.take(), Some(b"second WAD".to_vec()));
    start(&fresh, "resume").await;
    start(&fresh, "startup").await;
    assert!(
        fresh.ui.wad.borrow().is_none(),
        "consumed requests never reopen on later lifecycle events"
    );
    let reopened = replacement(root, &fresh).await;
    start(&reopened, "resume").await;
    assert!(
        reopened.ui.wad.borrow().is_none(),
        "consumption survives instance replacement"
    );
    matching_marker(first);
    assert!(
        first
            .history
            .borrow()
            .get_entries()
            .iter()
            .all(|entry| entry["type"] == "custom"),
        "game state is hidden custom data, not model or transcript messages"
    );
    shutdown(&fresh).await;
    shutdown(&reopened).await;
}
