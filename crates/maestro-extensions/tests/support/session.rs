//! The session operands a test runner serves, built from real in-memory owners.
#![cfg(test)]
use maestro_catalog::ModelRegistry;
use maestro_credentials::{AuthStorage, AuthStorageData};
use maestro_extensions::runner::ExtensionRunnerSession;
use maestro_session::SessionManager;
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// Builds a runner session over an in-memory history and catalog, returning the
/// writable history handle the application keeps.
pub fn runner_session(cwd: &str) -> (ExtensionRunnerSession, Rc<RefCell<SessionManager>>) {
    let history = Rc::new(RefCell::new(SessionManager::in_memory(Some(cwd)).unwrap()));
    let auth = Arc::new(AuthStorage::in_memory(AuthStorageData::default()));
    let session = ExtensionRunnerSession {
        cwd: cwd.into(),
        session_manager: history.clone(),
        model_registry: maestro_models::SharedHandle::new(ModelRegistry::in_memory(auth).unwrap()),
    };
    (session, history)
}
