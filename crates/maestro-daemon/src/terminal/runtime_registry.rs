//! Live process runtimes keyed by durable terminal identity.
use crate::pane::runtime::PaneRuntime;
use maestro_workspace::terminal_id::TerminalId;
use std::collections::HashMap;

/// Workspace-owned live terminals, separate from pure workspace state.
#[derive(Default)]
pub struct TerminalRuntimeRegistry {
    /// Concrete process and terminal owners.
    runtimes: HashMap<TerminalId, PaneRuntime>,
}

impl TerminalRuntimeRegistry {
    /// Creates an empty runtime registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Borrows a runtime by its durable terminal identity.
    #[must_use]
    pub fn get(&self, terminal_id: &TerminalId) -> Option<&PaneRuntime> {
        self.runtimes.get(terminal_id)
    }

    /// Replaces a runtime, returning the previous owner to the caller.
    pub fn insert(&mut self, terminal_id: TerminalId, runtime: PaneRuntime) -> Option<PaneRuntime> {
        self.runtimes.insert(terminal_id, runtime)
    }

    /// Removes a runtime without shutting it down while the map is borrowed.
    pub fn remove(&mut self, terminal_id: &TerminalId) -> Option<PaneRuntime> {
        self.runtimes.remove(terminal_id)
    }

    /// Borrows every retained runtime.
    pub fn values(&self) -> impl Iterator<Item = &PaneRuntime> {
        self.runtimes.values()
    }

    /// Borrows retained identity and runtime pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&TerminalId, &PaneRuntime)> {
        self.runtimes.iter()
    }

    /// Updates every reader's handoff state, logging individual failures.
    pub async fn set_handoff_readers_paused(&self, paused: bool) {
        for runtime in self.runtimes.values() {
            if let Err(error) = runtime.set_handoff_reader_paused(paused).await {
                tracing::warn!(%error, "failed to update pane handoff reader");
            }
        }
    }

    /// Takes responsibility for every imported runtime's process session.
    pub fn assume_handoff_ownership(&mut self) {
        for runtime in self.runtimes.values_mut() {
            runtime.assume_handoff_ownership();
        }
    }

    /// Number of retained live runtime owners.
    #[must_use]
    pub fn len(&self) -> usize {
        self.runtimes.len()
    }

    /// Whether no runtime is retained.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.runtimes.is_empty()
    }

    /// Nudges every child using the shared process actor's geometry operation.
    pub fn nudge_child_redraw_after_handoff(&self) {
        for runtime in self.runtimes.values() {
            runtime.nudge_child_redraw_after_handoff();
        }
    }

    /// Transfers every retained runtime out of this registry for handoff.
    pub fn drain_for_handoff(&mut self) -> impl Iterator<Item = (TerminalId, PaneRuntime)> + '_ {
        self.runtimes.drain()
    }
}

impl From<HashMap<TerminalId, PaneRuntime>> for TerminalRuntimeRegistry {
    fn from(runtimes: HashMap<TerminalId, PaneRuntime>) -> Self {
        Self { runtimes }
    }
}
