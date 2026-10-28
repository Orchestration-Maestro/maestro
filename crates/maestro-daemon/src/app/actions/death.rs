//! Pane-exit topology and retained-state cleanup.

use crate::app::state::{DaemonState, Mode};
use maestro_workspace::layout::PaneId;
use std::collections::HashSet;

impl<C: FnMut() -> u128> DaemonState<C> {
    /// Applies pane death without closing the other workspaces in its repository group.
    pub fn handle_pane_died(&mut self, pane: PaneId) {
        self.pending_agent_notifications.remove(&pane);
        self.remove_plugin_pane_records(&HashSet::from([pane]));
        let Some(index) = self
            .topology
            .workspaces
            .iter()
            .position(|workspace| workspace.find_tab_index_for_pane(pane).is_some())
        else {
            tracing::warn!(pane = pane.raw(), "pane death for unknown pane");
            return;
        };
        if self
            .selection
            .as_ref()
            .is_some_and(|selection| selection.pane_id == pane)
        {
            self.clear_selection();
        }
        let terminals = self.topology.workspaces[index]
            .tabs
            .iter()
            .flat_map(|tab| tab.panes.values())
            .map(|pane| pane.attached_terminal_id.clone())
            .collect();
        self.pane_id_aliases.retain(|_, alias| *alias != pane);
        self.public_pane_id_aliases
            .retain(|_, alias| *alias != pane);
        let (active, selected) = self.selection_identities();
        if self.topology.workspaces[index].remove_pane(pane) {
            self.topology.workspaces.remove(index);
            self.restore_workspace_selection_after_removal(active.as_deref(), selected.as_deref());
            if self.topology.workspaces.is_empty() && self.mode == Mode::Terminal {
                self.mode = Mode::Navigate;
            }
        }
        self.remove_unattached_terminal_ids(terminals);
        self.session_dirty = true;
    }

    /// Restores live identities after a workspace left the list, falling back to clamped
    /// previous positions.
    pub(crate) fn restore_workspace_selection_after_removal(
        &mut self,
        active: Option<&str>,
        selected: Option<&str>,
    ) {
        let workspaces = &self.topology.workspaces;
        if workspaces.is_empty() {
            self.active = None;
            self.selected = 0;
            return;
        }
        self.active = active
            .and_then(|id| workspaces.iter().position(|workspace| workspace.id == id))
            .or(self.active)
            .map(|index| index.min(workspaces.len() - 1));
        self.selected = selected
            .and_then(|id| workspaces.iter().position(|workspace| workspace.id == id))
            .unwrap_or(self.selected)
            .min(workspaces.len() - 1);
    }
}
