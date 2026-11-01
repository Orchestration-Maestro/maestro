//! The session operands a test runner serves, built from real in-memory owners.
#![cfg(test)]
use maestro_catalog::ModelRegistry;
use maestro_credentials::{AuthStorage, AuthStorageData};
use maestro_extensions::runner::ExtensionRunnerSession;
use maestro_models::SharedHandle;
use maestro_session::SessionManager;
use std::{cell::RefCell, rc::Rc};

/// Builds a runner session over an in-memory history and catalog, returning the
/// writable history handle the application keeps.
pub fn runner_session(cwd: &str) -> (ExtensionRunnerSession, Rc<RefCell<SessionManager>>) {
    runner_session_with_config(cwd, SharedHandle::new(NoCommands))
}

/// Builds a runner session with the caller's configured credential commands.
pub fn runner_session_with_config(
    cwd: &str,
    config: maestro_catalog::ConfigValues,
) -> (ExtensionRunnerSession, Rc<RefCell<SessionManager>>) {
    let history = Rc::new(RefCell::new(SessionManager::in_memory(Some(cwd)).unwrap()));
    let auth = SharedHandle::new(AuthStorage::in_memory(AuthStorageData::default()));
    let session = ExtensionRunnerSession {
        cwd: cwd.into(),
        session_manager: history.clone(),
        model_registry: maestro_models::SharedHandle::new(
            ModelRegistry::in_memory(auth, config).unwrap(),
        ),
    };
    (session, history)
}

/// Configured-value commands that never run: no extension test configures one.
pub(crate) struct NoCommands;

impl maestro_credentials::ConfigValueOperations for NoCommands {
    fn execute(&self, _: &str) -> Option<Vec<u8>> {
        panic!("no helper requested")
    }
}
