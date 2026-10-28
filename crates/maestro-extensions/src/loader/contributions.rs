//! Load-time inputs offered by base package factories.
use crate::{
    EventError, ExtensionAPI, ExtensionExecution,
    types::{ExtensionRuntime, read, write},
};
use serde_json::Value;
use std::collections::HashSet;

/// The host-admitted base paths and the attributed inputs offered during their loading.
pub(crate) struct LoadTimeState {
    /// Only these registration owners may contribute.
    paths: HashSet<String>,
    /// Inputs retain publication order and their original owner.
    pending: Vec<(String, Value)>,
}

impl<E: ExtensionExecution> ExtensionRuntime<E> {
    /// Opens the load-time channel for the supplied base extension paths.
    pub fn begin_load_time(&self, paths: Vec<String>) {
        *write(&self.load_time) = Some(LoadTimeState {
            paths: paths.into_iter().collect(),
            pending: Vec::new(),
        });
    }

    /// Closes the channel and returns its attributed inputs in publication order.
    #[must_use]
    pub fn finish_load_time(&self) -> Vec<(String, Value)> {
        write(&self.load_time)
            .take()
            .map_or_else(Vec::new, |state| state.pending)
    }
}

impl<E: ExtensionExecution> ExtensionAPI<E> {
    /// Offers package sources, base prompts, model files or base settings during base loading.
    /// The application validates the JSON input and reports a malformed contribution as a
    /// load error attributed to this extension.
    ///
    /// # Errors
    /// Returns an error outside the admitted base loading window or after invalidation.
    pub fn contribute_load_time(&self, contributions: Value) -> Result<(), EventError> {
        let owner = self.owner()?;
        let path = read(&owner).path.clone();
        let mut slot = write(&self.runtime.load_time);
        let Some(state) = slot.as_mut().filter(|state| state.paths.contains(&path)) else {
            return Err(
                "Load-time contributions are only available during base package loading".into(),
            );
        };
        state.pending.push((path, contributions));
        Ok(())
    }
}
