//! Coalesced render work retaining pane damage and title-change origins.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

use maestro_workspace::tiling::PaneId;

/// Render work accumulated since the last drain.
#[derive(Debug, Default)]
pub struct RenderRequest {
    /// Work not attributed to pane damage or title changes.
    pub generic: bool,
    /// Panes whose terminal content changed.
    pub pty_sources: HashSet<PaneId>,
    /// Panes whose terminal titles changed, independently of content damage.
    pub terminal_title_sources: HashSet<PaneId>,
}

/// Coalesces render requests so consumers can defer hidden pane damage.
#[derive(Debug, Default)]
pub struct RenderSignal {
    /// Lock-free observation of accumulated work.
    pending: AtomicBool,
    /// Request origins and the caller's currently visible pane set.
    state: Mutex<RenderSignalState>,
}

/// Origin sets protected together with the pending flag's transitions.
#[derive(Debug, Default)]
struct RenderSignalState {
    /// Work waiting to be taken.
    request: RenderRequest,
    /// Panes whose damage must wake the consumer immediately.
    immediate_pty_sources: HashSet<PaneId>,
}

impl RenderSignal {
    /// Creates an empty signal with no immediately visible panes.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Observes whether any work is pending without draining it.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.pending.load(Ordering::Acquire)
    }

    /// Adds work that cannot be deferred as hidden pane damage.
    pub fn request_generic(&self) {
        let mut state = self.lock();
        state.request.generic = true;
        self.pending.store(true, Ordering::Release);
    }

    /// Records pane damage, returning whether the consumer should wake.
    /// A new visible source wakes even when hidden damage is already pending.
    #[must_use]
    pub fn request_pty(&self, pane_id: PaneId) -> bool {
        let mut state = self.lock();
        let added = state.request.pty_sources.insert(pane_id);
        let visible = added && state.immediate_pty_sources.contains(&pane_id);
        let became_pending = !self.pending.swap(true, Ordering::AcqRel);
        became_pending || visible
    }

    /// Replaces the caller-supplied set of immediately visible panes.
    pub fn set_immediate_pty_sources(&self, sources: HashSet<PaneId>) {
        self.lock().immediate_pty_sources = sources;
    }

    /// Reports generic, title or visible pane work without draining it.
    #[must_use]
    pub fn has_immediate_work(&self) -> bool {
        let state = self.lock();
        state.request.generic
            || !state.request.terminal_title_sources.is_empty()
            || !state
                .request
                .pty_sources
                .is_disjoint(&state.immediate_pty_sources)
    }

    /// Records a title change separately from terminal damage.
    /// Returns whether pending work or this title source is newly recorded.
    #[must_use]
    pub fn request_terminal_title(&self, pane_id: PaneId) -> bool {
        let mut state = self.lock();
        let added = state.request.terminal_title_sources.insert(pane_id);
        let became_pending = !self.pending.swap(true, Ordering::AcqRel);
        became_pending || added
    }

    /// Copies pending title origins without consuming any render work.
    #[must_use]
    pub fn pending_terminal_title_sources(&self) -> HashSet<PaneId> {
        self.lock().request.terminal_title_sources.clone()
    }

    /// Drains accumulated work atomically, retaining the visibility policy.
    #[must_use]
    pub fn take(&self) -> RenderRequest {
        let mut state = self.lock();
        self.pending.store(false, Ordering::Release);
        std::mem::take(&mut state.request)
    }

    /// Recovers owned render state if a previous caller unwound while holding it.
    fn lock(&self) -> MutexGuard<'_, RenderSignalState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests;
