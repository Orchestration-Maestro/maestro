//! Pane-owned metadata and focus cleanup after topology removal.

use crate::app::model::{DaemonState, Mode};
use maestro_workspace::{terminal_id::TerminalId, tiling::PaneId};
use std::collections::HashSet;

impl<C: FnMut() -> u128> DaemonState<C> {
    /// Removes pane-owned records and invalidates focus or copy targets.
    pub(crate) fn remove_plugin_pane_records(&mut self, panes: &HashSet<PaneId>) {
        if self
            .copy_mode
            .as_ref()
            .is_some_and(|copy| panes.contains(&copy.pane_id))
        {
            self.clear_selection();
            self.copy_mode = None;
            if self.mode == Mode::Copy {
                self.mode = self.active.map_or(Mode::Navigate, |_| Mode::Terminal);
            }
        }
        if self
            .previous_pane_focus
            .as_ref()
            .is_some_and(|focus| panes.contains(&focus.pane_id))
        {
            self.previous_pane_focus = None;
        }
        self.plugin_panes.retain(|pane, _| !panes.contains(pane));
    }

    /// Queues detached terminals once without rescanning topology per candidate.
    pub(crate) fn remove_unattached_terminal_ids(&mut self, candidates: Vec<TerminalId>) {
        let attached: HashSet<_> = self
            .topology
            .workspaces
            .iter()
            .flat_map(|workspace| &workspace.tabs)
            .flat_map(|tab| tab.panes.values())
            .map(|pane| &pane.attached_terminal_id)
            .collect();
        let mut queued: HashSet<_> = self.terminal_runtime_shutdowns.iter().cloned().collect();
        for terminal in candidates {
            if !attached.contains(&terminal)
                && self.terminals.remove(&terminal).is_some()
                && queued.insert(terminal.clone())
            {
                self.terminal_runtime_shutdowns.push(terminal);
            }
        }
    }
}
