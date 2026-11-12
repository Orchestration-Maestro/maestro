//! Supplied native notification identities and explicit delivery for author tests.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::{bus_host::Emitter, controlled::Controlled};
use crate::{ExtensionResult, component_adapter::Exports};
use std::sync::{Arc, Mutex, Weak};

/// One original callback identity, released once when its registration ends.
pub struct Watcher(
    /// The live callback identity, taken once on close.
    pub Arc<Mutex<Option<u32>>>,
    /// Component callbacks are released after the current guest call returns.
    Option<Arc<Mutex<Vec<u32>>>>,
);
impl Watcher {
    /// Stops this supplied registration and releases the retained author callback.
    pub fn close(&self) {
        let id = self.0.lock().unwrap().take();
        if let Some(id) = id {
            if let Some(retired) = &self.1 {
                retired.lock().unwrap().push(id);
            } else {
                crate::component_adapter::release(id);
            }
        }
    }
}
impl Drop for Watcher {
    fn drop(&mut self) {
        self.close();
    }
}
/// Supplied watch outcomes and weak observations, never registration owners.
#[derive(Default)]
pub struct Bank {
    /// Literal paths passed to creation.
    pub paths: Vec<String>,
    /// Weak references cannot keep a released registration alive.
    pub watches: Vec<Weak<Mutex<Option<u32>>>>,
    /// Selected creation failure.
    pub error: Option<String>,
    /// Component callback releases deferred until re-entry is possible.
    retired: Arc<Mutex<Vec<u32>>>,
}
impl Bank {
    /// Admits a supplied identity or returns the selected failure.
    pub fn watch(&mut self, path: String, id: u32) -> ExtensionResult<Watcher> {
        self.paths.push(path);
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        let callback = Arc::new(Mutex::new(Some(id)));
        self.watches.push(Arc::downgrade(&callback));
        Ok(Watcher(callback, None))
    }

    /// Admits a component callback using the same notification state.
    ///
    /// # Errors
    /// Returns the selected watch creation error, if one is configured.
    pub fn component_watch(&mut self, path: String, id: u32) -> ExtensionResult<Watcher> {
        let mut watcher = self.watch(path, id)?;
        watcher.1 = Some(self.retired.clone());
        Ok(watcher)
    }

    /// Returns a callback only while the original registration remains open.
    fn callback(&self, index: usize) -> Option<u32> {
        self.watches[index]
            .upgrade()
            .and_then(|watch| *watch.lock().unwrap())
    }
}
impl Controlled {
    /// Delivers one notification only while its original registration remains open.
    pub fn notify_watch(&self, index: usize) -> ExtensionResult<()> {
        let id = self.observed().native_watch.callback(index);
        if let Some(id) = id {
            Exports::new(self.clone()).invoke_watch(id, &Emitter { key: 0 })?;
        }
        Ok(())
    }
}

use super::host::State;
use crate::bindings::host_side::maestro::extension::host;
use wasmtime::component::Resource;
impl host::HostFsWatcher for State {
    fn close(&mut self, watch: Resource<Watcher>) -> wasmtime::Result<ExtensionResult<()>> {
        self.table.get(&watch)?.close();
        Ok(Ok(()))
    }
    fn drop(&mut self, watch: Resource<Watcher>) -> wasmtime::Result<()> {
        self.release(watch)
    }
}
