//! Workspace removal preserves logical active selection.

#[cfg(test)]
mod tests;

use crate::app::state::{DaemonState, Mode};
use maestro_workspace::layout::PaneId;

impl<C: FnMut() -> u128> DaemonState<C> {
    /// Retires one non-final tab and its detached pane and terminal records.
    pub(crate) fn retire_tab(&mut self, workspace: usize, tab: usize) -> bool {
        let Some(workspace) = self.topology.workspaces.get_mut(workspace) else {
            return false;
        };
        let Some(closing) = workspace.tabs.get(tab) else {
            return false;
        };
        let panes = closing.panes.keys().copied().collect();
        let terminals = closing
            .panes
            .values()
            .map(|pane| pane.attached_terminal_id.clone())
            .collect();
        if !workspace.close_tab(tab) {
            return false;
        }
        self.remove_plugin_pane_records(&panes);
        self.remove_unattached_terminal_ids(terminals);
        true
    }

    /// Whether closing `pane` would leave its workspace without any pane.
    #[must_use]
    pub fn close_pane_would_close_workspace(&self, workspace: usize, pane: PaneId) -> bool {
        self.topology
            .workspaces
            .get(workspace)
            .is_some_and(|target| {
                target.tabs.len() <= 1
                    && target
                        .find_tab_index_for_pane(pane)
                        .is_some_and(|tab| target.tabs[tab].layout.pane_count() <= 1)
            })
    }

    /// Closes the focused pane, returning whether confirmation deferred the close.
    pub fn close_pane(&mut self) -> bool {
        let focused = self.active.and_then(|index| {
            let pane = self.topology.workspaces.get(index)?.focused_pane_id()?;
            Some((index, pane))
        });
        if let Some((index, pane)) = focused
            && self.close_pane_would_close_workspace(index, pane)
            && self.confirm_implicit_worktree_group_close(index)
        {
            return true;
        }
        self.clear_selection();
        self.session_dirty = true;
        if let Some((index, pane)) = focused {
            self.close_pane_in_workspace(index, pane);
        }
        false
    }

    /// Closes a pane with its pane-owned records and terminal, reporting whether its
    /// workspace closed with it because the pane was the last one there.
    pub fn close_pane_in_workspace(&mut self, workspace: usize, pane: PaneId) -> bool {
        let Some(target) = self.topology.workspaces.get_mut(workspace) else {
            return false;
        };
        let terminals = target.terminal_id(pane).cloned().into_iter().collect();
        let close_workspace = target.close_pane(pane);
        self.remove_plugin_pane_records(&std::collections::HashSet::from([pane]));
        if close_workspace {
            self.selected = workspace;
            self.close_selected_workspace();
        } else {
            self.remove_unattached_terminal_ids(terminals);
        }
        close_workspace
    }

    /// Closes the active tab, returning whether confirmation deferred the close.
    pub fn close_tab(&mut self) -> bool {
        if let Some(index) = self.active
            && self
                .topology
                .workspaces
                .get(index)
                .is_some_and(|workspace| workspace.tabs.len() <= 1)
            && self.confirm_implicit_worktree_group_close(index)
        {
            return true;
        }
        self.clear_selection();
        self.session_dirty = true;
        let Some(index) = self.active else {
            return false;
        };
        let Some(workspace) = self.topology.workspaces.get_mut(index) else {
            return false;
        };
        if workspace.tabs.len() <= 1 {
            self.selected = index;
            self.close_selected_workspace();
            return false;
        }
        let tab = workspace.active_tab;
        self.retire_tab(index, tab);
        false
    }

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
        let indices = self.workspace_close_indices(self.selected);
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
    /// Retains a parent-group confirmation request without changing topology.
    pub(crate) fn confirm_implicit_worktree_group_close(&mut self, index: usize) -> bool {
        if self.confirm_close.0 && self.workspace_close_indices(index).len() >= 2 {
            self.selected = index;
            self.mode = Mode::ConfirmClose;
            true
        } else {
            false
        }
    }

    /// Selects the parent repository group, or only the selected linked workspace.
    fn workspace_close_indices(&self, index: usize) -> Vec<usize> {
        let group = self
            .topology
            .workspaces
            .get(index)
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
        vec![index]
    }
}
