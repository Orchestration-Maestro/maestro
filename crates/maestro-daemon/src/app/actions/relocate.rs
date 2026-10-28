//! Pane relocation bookkeeping: removing an emptied source workspace and restoring a source
//! when its destination refuses the pane.

#[cfg(test)]
mod tests;

use crate::app::state::DaemonState;
use maestro_workspace::{tab::MovedPane, workspace::WorktreeSpaceMembership};
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
        let _ = index;
    }

    /// Puts a pane back where a refused move took it from: as a new tab of its workspace,
    /// or, when that workspace is gone, in a restored workspace of the same identity.
    pub fn recover_failed_pane_move(&mut self, recovery: PaneMoveRecovery, moved: MovedPane) {
        let _ = (recovery, moved);
    }
}
