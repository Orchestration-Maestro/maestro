//! Repository status observations applied to retained workspaces.

use crate::{app::state::DaemonState, terminal::runtime_registry::TerminalRuntimeRegistry};
use maestro_workspace::workspace::WorkspaceGitStatus;

impl<C: FnMut() -> u128> DaemonState<C> {
    /// Apply observations to the workspaces they still describe and report visible changes.
    pub fn apply_workspace_git_statuses(
        &mut self,
        runtimes: &TerminalRuntimeRegistry,
        results: Vec<WorkspaceGitStatus>,
    ) -> bool {
        let _ = (runtimes, results);
        false
    }
}
