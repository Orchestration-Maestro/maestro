//! One retained placement record for the two controlled host adapters.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::host::State;
use crate::OverlayOptions;
use crate::bindings::host_side::maestro::extension::host;
use std::sync::{Arc, Mutex};
use wasmtime::component::Resource;

/// The original placement record shared with a shown overlay.
#[derive(Clone)]
pub struct Options(Arc<Mutex<OverlayOptions>>);
impl Options {
    /// Validates and retains placement.
    pub fn new(encoded: &str) -> Result<Self, String> {
        serde_json::from_str(encoded)
            .map(|options| Self(Arc::new(Mutex::new(options))))
            .map_err(|error| error.to_string())
    }
    /// Reads current placement.
    pub fn get(&self) -> Result<String, String> {
        serde_json::to_string(&*self.0.lock().unwrap()).map_err(|error| error.to_string())
    }
    /// Replaces the original placement record.
    pub fn set(&self, encoded: &str) -> Result<(), String> {
        let options = serde_json::from_str(encoded).map_err(|error| error.to_string())?;
        *self.0.lock().unwrap() = options;
        Ok(())
    }
}

/// Creates and lends one placement owner.
pub fn create(
    state: &mut State,
    encoded: &str,
) -> wasmtime::Result<Result<Resource<Options>, String>> {
    match Options::new(encoded) {
        Ok(options) => Ok(Ok(state.push(options)?)),
        Err(error) => Ok(Err(error)),
    }
}
impl host::HostOverlayOptions for State {
    fn get(&mut self, this: Resource<Options>) -> wasmtime::Result<Result<String, String>> {
        Ok(self.table.get(&this)?.get())
    }
    fn set(
        &mut self,
        this: Resource<Options>,
        encoded: String,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(self.table.get(&this)?.set(&encoded))
    }
    fn share(&mut self, this: Resource<Options>) -> wasmtime::Result<Resource<Options>> {
        let shared = self.table.get(&this)?.clone();
        self.push(shared)
    }
    fn set_visible(
        &mut self,
        _: Resource<Options>,
        visible: Option<Resource<super::host::Identity>>,
    ) -> wasmtime::Result<Result<(), String>> {
        self.observed
            .log(format!("overlay visible {}", visible.is_some()));
        Ok(Ok(()))
    }
    fn drop(&mut self, this: Resource<Options>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

/// A shown overlay; the component test host shows none.
pub struct Handle;
impl host::HostOverlayHandle for State {
    fn request(
        &mut self,
        _: Resource<Handle>,
        _: host::OverlayRequest,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        self.push(super::host::Route {
            steps: std::collections::VecDeque::default(),
            running: None,
            result: Some(Err("no overlay is shown in the component test host".into())),
            rows: None,
        })
    }
    fn drop(&mut self, this: Resource<Handle>) -> wasmtime::Result<()> {
        self.release(this)
    }
}
