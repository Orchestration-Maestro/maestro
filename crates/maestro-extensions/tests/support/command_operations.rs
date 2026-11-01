//! Plain command operations for controlled native contexts.
#![cfg(test)]
use maestro_extensions::{CommandContextOperations, EventError, types::SessionChangeResult};
use maestro_models::BoxFuture;
use std::rc::Rc;

/// Command operations that answer plainly without touching a session.
#[must_use]
pub fn silent_command_operations() -> CommandContextOperations {
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
