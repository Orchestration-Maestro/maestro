//! Workspace removal preserves logical active selection.

use crate::app::state::DaemonState;

impl<C: FnMut() -> u128> DaemonState<C> {
    /// Closes the selected workspace, retaining the active workspace if it survives.
    pub fn close_selected_workspace(&mut self) {
        if self.topology.workspaces.is_empty() {
            return;
        }
        let active = self
            .active
            .and_then(|index| self.topology.workspaces.get(index))
            .map(|workspace| workspace.id.clone());
        self.clear_selection();
        self.session_dirty = true;
        let indices = self.workspace_close_indices();
        let panes = indices
            .iter()
            .flat_map(|index| &self.topology.workspaces[*index].tabs)
            .flat_map(|tab| tab.panes.keys().copied())
            .collect();
        let terminals = indices
            .iter()
            .flat_map(|index| &self.topology.workspaces[*index].tabs)
            .flat_map(|tab| tab.panes.values())
            .map(|pane| pane.attached_terminal_id.clone())
            .collect();
        self.remove_plugin_pane_records(&panes);
        for index in indices.into_iter().rev() {
            self.topology.workspaces.remove(index);
        }
        self.remove_unattached_terminal_ids(terminals);
        self.selected = active
            .and_then(|id| {
                self.topology
                    .workspaces
                    .iter()
                    .position(|workspace| workspace.id == id)
            })
            .unwrap_or(self.selected)
            .min(self.topology.workspaces.len().saturating_sub(1));
        self.active = (!self.topology.workspaces.is_empty()).then_some(self.selected);
    }
    /// Selects the parent repository group, or only the selected linked workspace.
    fn workspace_close_indices(&self) -> Vec<usize> {
        let group = self
            .topology
            .workspaces
            .get(self.selected)
            .and_then(|workspace| workspace.worktree_space.as_ref())
            .filter(|space| !space.is_linked_worktree);
        if let Some(group) = group {
            return self
                .topology
                .workspaces
                .iter()
                .enumerate()
                .filter_map(|(index, workspace)| {
                    workspace
                        .worktree_space
                        .as_ref()
                        .is_some_and(|space| space.key == group.key)
                        .then_some(index)
                })
                .collect();
        }
        vec![self.selected]
    }
}
