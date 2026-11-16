//! Controlled settings-owner and warning fixtures.

use std::sync::Arc;

use maestro_settings::{InMemorySettingsStorage, SettingsManager, SettingsScope, SettingsStorage};
use serde_json::Value;

pub(crate) use crate::tests::block_on;

/// Replace raw storage through its existing seam.
pub fn put(storage: &dyn SettingsStorage, scope: SettingsScope, text: &str) {
    storage
        .with_lock(scope, &mut |_| Ok(Some(text.to_owned())))
        .unwrap();
}

/// Construct a supplied manager with independently controlled scopes.
pub fn manager(global: &Value, project: &Value) -> (SettingsManager, Arc<InMemorySettingsStorage>) {
    let storage = Arc::new(InMemorySettingsStorage::new());
    put(
        &*storage,
        SettingsScope::Global,
        &serde_json::to_string(&global).unwrap(),
    );
    put(
        &*storage,
        SettingsScope::Project,
        &serde_json::to_string(&project).unwrap(),
    );
    (SettingsManager::from_storage(storage.clone()), storage)
}

/// Capture warning message and error fields around one synchronous operation.
pub fn capture<T>(action: impl FnOnce() -> T) -> (T, String) {
    let output = maestro_test_tracing::Output::default();
    let writer = output.clone();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_max_level(tracing::Level::WARN)
        .with_writer(move || writer.clone())
        .finish();
    let result = maestro_test_tracing::with_default(subscriber, action);
    (result, output.text())
}
