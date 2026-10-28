//! Workspace and pane focus navigation with retained logical history.

use crate::app::state::{DaemonState, Mode, PaneFocusTarget};
use maestro_workspace::layout::PaneId;

impl<C: FnMut() -> u128> DaemonState<C> {
    /// Selects a workspace and marks the panes in its active tab seen.
    pub fn switch_workspace(&mut self, index: usize) {
        let Some(workspace) = self.topology.workspaces.get(index) else {
            return;
        };
        let tab = workspace.active_tab;
        self.switch_workspace_tab(index, tab);
    }

    /// Selects a valid workspace and tab, marking its panes seen.
    pub fn switch_workspace_tab(&mut self, workspace: usize, tab: usize) -> bool {
        let previous = self.current_pane_focus_target();
        let Some(target) = self.topology.workspaces.get_mut(workspace) else {
            return false;
        };
        if tab >= target.tabs.len() {
            return false;
        }
        target.switch_tab(tab);
        self.active = Some(workspace);
        self.selected = workspace;
        self.session_dirty = true;
        if previous != self.current_pane_focus_target() {
            self.previous_pane_focus = previous;
        }
        if self.copy_mode.is_some() {
            self.sync_copy_mode_with_focus();
        } else {
            self.clear_selection();
        }
        true
    }

    /// Returns the active pane's logical identity.
    #[must_use]
    pub fn current_pane_focus_target(&self) -> Option<PaneFocusTarget> {
        let workspace = self.topology.workspaces.get(self.active?)?;
        Some(PaneFocusTarget {
            workspace_id: workspace.id.clone(),
            pane_id: workspace.focused_pane_id()?,
        })
    }

    /// Resolves a retained target only while its workspace and pane remain live.
    #[must_use]
    pub fn pane_focus_target_indices(&self, target: &PaneFocusTarget) -> Option<(usize, usize)> {
        let workspace = self
            .topology
            .workspaces
            .iter()
            .position(|workspace| workspace.id == target.workspace_id)?;
        let tab = self.topology.workspaces[workspace].find_tab_index_for_pane(target.pane_id)?;
        Some((workspace, tab))
    }

    /// Focuses a live pane across workspace and tab boundaries.
    pub fn focus_pane_in_workspace(&mut self, workspace: usize, pane: PaneId) -> bool {
        let Some(target) = self.topology.workspaces.get(workspace) else {
            return false;
        };
        let Some(tab) = target.find_tab_index_for_pane(pane) else {
            return false;
        };
        let previous = self.current_pane_focus_target();
        if previous.as_ref().is_some_and(|target| {
            target.workspace_id == self.topology.workspaces[workspace].id && target.pane_id == pane
        }) {
            return false;
        }
        if self.copy_mode.is_some() {
            self.clear_copy_mode_selection();
        }
        self.switch_workspace_tab(workspace, tab);
        self.topology.workspaces[workspace].tabs[tab]
            .layout
            .focus_pane(pane);
        self.previous_pane_focus = previous;
        self.sync_copy_mode_with_focus();
        true
    }

    /// Marks every pane of the active tab seen.
    pub fn mark_active_tab_seen(&mut self) {
        let active = self
            .active
            .and_then(|index| self.topology.workspaces.get_mut(index));
        if let Some(workspace) = active {
            // Reselecting the active tab marks its panes seen and changes nothing else.
            workspace.switch_tab(workspace.active_tab);
        }
    }

    /// Returns to terminal input after a focus change, keeping copy context only on its pane.
    pub fn settle_terminal_mode_after_focus(&mut self) {
        self.mode = Mode::Terminal;
        self.sync_copy_mode_with_focus();
    }

    /// Toggles focus to the previous live pane, discarding stale history.
    pub fn last_pane(&mut self) {
        let Some(target) = self.previous_pane_focus.as_ref() else {
            return;
        };
        let Some((workspace, tab)) = self.pane_focus_target_indices(target) else {
            self.previous_pane_focus = None;
            return;
        };
        let pane = target.pane_id;
        let current = self.current_pane_focus_target();
        if current.as_ref() == Some(target) {
            self.previous_pane_focus = None;
            return;
        }
        self.switch_workspace_tab(workspace, tab);
        self.topology.workspaces[workspace].tabs[tab]
            .layout
            .focus_pane(pane);
        self.previous_pane_focus = current;
    }

    /// Clears terminal text selection.
    pub fn clear_selection(&mut self) {
        self.selection = None;
        self.selection_autoscroll = None;
    }

    /// Clears selection while retaining the copy pane for return navigation.
    pub fn clear_copy_mode_selection(&mut self) {
        self.clear_selection();
        if let Some(copy) = self.copy_mode.as_mut() {
            copy.selection = None;
        }
    }

    /// Restores copy context only when its retained pane is currently focused.
    pub fn sync_copy_mode_with_focus(&mut self) {
        let Some(copy) = self.copy_mode.as_ref() else {
            return;
        };
        if self
            .current_pane_focus_target()
            .is_some_and(|target| target.pane_id == copy.pane_id)
        {
            self.mode = Mode::Copy;
        } else {
            self.clear_copy_mode_selection();
            self.mode = if self.active.is_some() {
                Mode::Terminal
            } else {
                Mode::Navigate
            };
        }
    }
}
