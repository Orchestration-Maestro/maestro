//! Live process observations over topology-owned workspaces.

use crate::terminal::{runtime_registry::TerminalRuntimeRegistry, state::TerminalState};
use maestro_workspace::{terminal_id::TerminalId, workspace::Workspace};
use std::{collections::HashMap, path::PathBuf};

/// Resolves identity from the first root process, falling back to retained topology.
#[must_use]
pub fn resolved_identity_cwd_from<S: std::hash::BuildHasher>(
    workspace: &Workspace,
    terminals: &HashMap<TerminalId, TerminalState, S>,
    runtimes: &TerminalRuntimeRegistry,
) -> Option<PathBuf> {
    workspace
        .tabs
        .first()
        .and_then(|tab| crate::tab::cwd_for_pane(tab, tab.root_pane, terminals, runtimes))
        .or_else(|| Some(workspace.identity_cwd.clone()))
}
