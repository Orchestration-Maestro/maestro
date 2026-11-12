//! Deadline-driven managed ownership reconciliation without another timer owner.

use crate::{app::model::DaemonState, terminal::model::TerminalState};
use maestro_workspace::tiling::PaneId;
use std::{collections::HashSet, time::Instant};

impl<C: FnMut() -> u128> DaemonState<C> {
    /// Earliest settling or launch deadline across the retained terminal owners.
    pub(crate) fn next_managed_agent_deadline(&self) -> Option<Instant> {
        self.terminals
            .values()
            .filter_map(TerminalState::next_managed_agent_deadline)
            .min()
    }

    /// Reconciles each terminal once and returns every changed attached viewport.
    pub(crate) fn reconcile_managed_agents_at(&mut self, now: Instant) -> Vec<(usize, PaneId)> {
        let changed: HashSet<_> = self
            .terminals
            .iter_mut()
            .filter_map(|(id, terminal)| {
                terminal
                    .reconcile_managed_agent_at(now, false)
                    .then(|| id.clone())
            })
            .collect();
        if changed.is_empty() {
            return Vec::new();
        }
        self.session_dirty = true;
        self.topology
            .workspaces
            .iter()
            .enumerate()
            .flat_map(|(index, workspace)| workspace.tabs.iter().map(move |tab| (index, tab)))
            .flat_map(|(index, tab)| {
                tab.panes
                    .iter()
                    .map(move |(pane, state)| (index, *pane, state))
            })
            .filter_map(|(index, pane, state)| {
                changed
                    .contains(&state.attached_terminal_id)
                    .then_some((index, pane))
            })
            .collect()
    }
}
