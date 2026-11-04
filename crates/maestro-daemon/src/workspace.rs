//! Live process observations over topology-owned workspaces.

/// Terminal-aware workspace attention and pane details.
pub mod aggregate;

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

/// Labels a workspace from admitted root-terminal metadata without probing its live process.
#[must_use]
pub fn display_name_from_terminals<S: std::hash::BuildHasher>(
    workspace: &Workspace,
    terminals: &HashMap<TerminalId, TerminalState, S>,
    home: Option<&std::path::Path>,
) -> String {
    let cwd = workspace
        .tabs
        .first()
        .and_then(|tab| tab.terminal_id(tab.root_pane))
        .and_then(|terminal| terminals.get(terminal))
        .map_or(&workspace.identity_cwd, |terminal| &terminal.cwd);
    workspace.display_name_for_cwd(cwd, home)
}

/// Admit native repository metadata once after successful workspace creation.
pub(crate) fn admit_git_identity(workspace: &mut Workspace, home: Option<&std::path::Path>) {
    use crate::git::discovery::{
        automatic_workspace_label, fallback_label_from_cwd, git_branch, git_space_metadata,
    };
    let cwd = &workspace.identity_cwd;
    let space = git_space_metadata(cwd);
    workspace.cached_auto_label = space.as_ref().map_or_else(
        || fallback_label_from_cwd(cwd, home),
        |space| automatic_workspace_label(cwd, &space.repo_root, home),
    );
    workspace.cached_git_status_key = space.as_ref().map_or_else(
        || cwd.clone(),
        crate::git::status::git_status_cache_key_for_space,
    );
    workspace.cached_git_branch = git_branch(cwd);
    workspace.cached_git_space = space;
}

/// Labels a workspace from the first root's live directory, preserving explicit names.
#[must_use]
pub fn display_name_from<S: std::hash::BuildHasher>(
    workspace: &Workspace,
    terminals: &HashMap<TerminalId, TerminalState, S>,
    runtimes: &TerminalRuntimeRegistry,
    home: Option<&std::path::Path>,
) -> String {
    if let Some(name) = &workspace.custom_name {
        return name.clone();
    }
    resolved_identity_cwd_from(workspace, terminals, runtimes).map_or_else(
        || "workspace".into(),
        |cwd| workspace.display_name_for_cwd(&cwd, home),
    )
}
