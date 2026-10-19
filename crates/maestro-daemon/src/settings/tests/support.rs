//! Controlled settings-owner and warning fixtures.

use std::{
    future::Future,
    io::{self, Write},
    sync::{Arc, Mutex},
};

use maestro_settings::{InMemorySettingsStorage, SettingsManager, SettingsScope, SettingsStorage};
use serde_json::Value;

/// Drive the settings owner's barrier without test deadlines.
pub fn block_on<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(future)
}

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

/// Thread-scoped tracing output, never a global subscriber.
#[derive(Clone, Default)]
struct Output(Arc<Mutex<Vec<u8>>>);

impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Capture warning message and error fields around one synchronous operation.
pub fn capture<T>(action: impl FnOnce() -> T) -> (T, String) {
    let output = Output::default();
    let writer = output.clone();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_max_level(tracing::Level::WARN)
        .with_writer(move || writer.clone())
        .finish();
    let result = tracing::subscriber::with_default(subscriber, action);
    let text = String::from_utf8(output.0.lock().unwrap().clone()).unwrap();
    (result, text)
}
