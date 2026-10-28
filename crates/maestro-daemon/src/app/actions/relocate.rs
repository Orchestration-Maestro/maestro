//! Pane relocation bookkeeping: removing an emptied source workspace and restoring a source
//! when its destination refuses the pane.

#[cfg(test)]
mod tests;

use crate::app::state::DaemonState;
use maestro_workspace::{
    tab::MovedPane,
    workspace::{WorkspaceTopology, WorktreeSpaceMembership},
};
use std::path::PathBuf;

/// What a move captures about its source so that a refused move can put the pane back.
#[derive(Debug, Clone)]
pub struct PaneMoveRecovery {
    /// Display position the source workspace had.
    pub workspace: usize,
    /// Stable identity of the source workspace.
    pub workspace_id: String,
    /// Explicit label of the source workspace.
    pub workspace_label: Option<String>,
    /// Explicit label of the source tab.
    pub tab_label: Option<String>,
    /// Worktree provenance of the source workspace.
    pub worktree_space: Option<WorktreeSpaceMembership>,
    /// Directory that gave the source workspace its identity.
    pub identity_cwd: PathBuf,
}

impl<C: FnMut() -> u128> DaemonState<C> {
    /// Removes a workspace left without tabs, keeping the active and selected workspaces.
    pub fn remove_emptied_workspace(&mut self, index: usize) {
        if index >= self.topology.workspaces.len() {
            return;
        }
        self.topology.workspaces.remove(index);
        let len = self.topology.workspaces.len();
        if len == 0 {
            self.active = None;
            self.selected = 0;
            return;
        }
        self.active = self
            .active
            .map(|active| position_after_removal(active, index, len));
        self.selected = position_after_removal(self.selected, index, len);
    }

    /// Puts a pane back where a refused move took it from: as a new tab of its workspace,
    /// or, when that workspace is gone, in a restored workspace of the same identity.
    pub fn recover_failed_pane_move(&mut self, recovery: PaneMoveRecovery, moved: MovedPane) {
        if let Some(index) = self
            .topology
            .workspaces
            .iter()
            .position(|workspace| workspace.id == recovery.workspace_id)
        {
            self.topology.workspaces[index]
                .create_tab_from_existing_pane(moved, recovery.tab_label);
        } else {
            self.restore_source_workspace(recovery, moved);
        }
        self.session_dirty = true;
    }

    /// Appends a workspace around a transferred pane, which becomes its only tab's only pane,
    /// and returns the workspace's display position.
    pub(crate) fn new_workspace_from_moved_pane(
        &mut self,
        cwd: PathBuf,
        label: Option<String>,
        tab_label: Option<String>,
        moved: MovedPane,
    ) -> usize {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let index =
            WorkspaceTopology::from_existing_pane(&mut self.topology, cwd, home.as_deref(), moved);
        let workspace = &mut self.topology.workspaces[index];
        workspace.custom_name = label;
        workspace.tabs[0].custom_name = tab_label;
        crate::workspace::admit_git_identity(workspace, home.as_deref());
        index
    }

    /// Rebuilds a removed source workspace around the refused pane, with its old identity, at
    /// its old display position.
    fn restore_source_workspace(&mut self, recovery: PaneMoveRecovery, moved: MovedPane) {
        let index = self.new_workspace_from_moved_pane(
            recovery.identity_cwd,
            recovery.workspace_label,
            recovery.tab_label,
            moved,
        );
        let mut workspace = self.topology.workspaces.remove(index);
        workspace.id = recovery.workspace_id;
        workspace.worktree_space = recovery.worktree_space;
        let at = recovery.workspace.min(self.topology.workspaces.len());
        self.active = self
            .active
            .map(|active| if active >= at { active + 1 } else { active });
        if self.selected >= at && !self.topology.workspaces.is_empty() {
            self.selected += 1;
        }
        self.topology.workspaces.insert(at, workspace);
    }
}

/// Where a position in a list of `len` entries lands after the entry at `removed` is taken
/// out: entries after it move up, and the removed entry's position goes to its successor.
fn position_after_removal(position: usize, removed: usize, len: usize) -> usize {
    match position.cmp(&removed) {
        std::cmp::Ordering::Less => position,
        std::cmp::Ordering::Equal => removed.min(len - 1),
        std::cmp::Ordering::Greater => position - 1,
    }
}
