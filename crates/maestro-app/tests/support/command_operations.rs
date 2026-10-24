//! Command-context operations a test binds when it exercises another binding.
#![cfg(not(target_arch = "wasm32"))]
use maestro_app::{BoxFuture, CommandContextOperations};
use maestro_extensions::{EventError, types::SessionChangeResult};
use std::rc::Rc;

/// Operations that answer plainly without touching a session.
#[must_use]
pub fn silent() -> CommandContextOperations {
    let uncancelled = || -> BoxFuture<Result<SessionChangeResult, EventError>> {
        Box::pin(std::future::ready(Ok(SessionChangeResult {
            cancelled: false,
        })))
    };
    CommandContextOperations {
        wait_for_idle: Rc::new(|| Box::pin(std::future::ready(Ok(())))),
        new_session: Rc::new(move |_| uncancelled()),
        fork: Rc::new(move |_, _| uncancelled()),
        navigate_tree: Rc::new(move |_, _| uncancelled()),
        switch_session: Rc::new(move |_, _| uncancelled()),
        reload: Rc::new(|| Box::pin(std::future::ready(Ok(())))),
    }
}
