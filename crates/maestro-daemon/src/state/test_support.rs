//! Setup entries for tests that need a topology without live processes.
//!
//! Each entry runs through the owners production uses, so the save mark and the retained
//! terminal metadata behave as they do for a created workspace. Reads and assertions go
//! through commands and events, never through these entries.

use super::App;
use crate::terminal::model::TerminalState;
use maestro_workspace::{
    spaces::WorktreeSpaceMembership,
    tiling::{PaneId, SplitRequest},
};

impl App {
    /// Test setup: adds a workspace in `cwd` whose root pane has no process.
    ///
    /// The first workspace added becomes the active one. Returns its root pane.
    pub fn seed_workspace(&mut self, cwd: &str) -> Option<PaneId> {
        self.state.edit_topology(|state| {
            let (pane, ()) = state
                .topology
                .new_with_tab(cwd.into(), None, |_, _| Ok(()))
                .ok()?;
            state.admit_seeded_terminals();
            if state.active.is_none() {
                state.active = Some(0);
            }
            Some(pane)
        })
    }

    /// Test setup: adds a tab without a process to the workspace at `workspace`.
    ///
    /// Returns the tab's root pane, or nothing when the workspace is missing.
    pub fn seed_tab(&mut self, workspace: usize) -> Option<PaneId> {
        self.state.edit_topology(|state| {
            let (pane, ()) = state
                .topology
                .create_tab_with_runtime(workspace, |_, _| Ok(()))?
                .ok()?;
            state.admit_seeded_terminals();
            Some(pane)
        })
    }

    /// Test setup: splits a pane of the workspace at `workspace` without a process.
    ///
    /// Returns the new pane, or nothing when the workspace or target is missing.
    pub fn seed_split(&mut self, workspace: usize, request: SplitRequest) -> Option<PaneId> {
        self.state.edit_topology(|state| {
            let (pane, ()) = state
                .topology
                .split_pane_with_runtime(workspace, request, |_, _| Ok(()))?
                .ok()?;
            state.admit_seeded_terminals();
            Some(pane)
        })
    }

    /// Test setup: joins the workspace at `workspace` to a repository group.
    pub fn seed_worktree_membership(
        &mut self,
        workspace: usize,
        membership: WorktreeSpaceMembership,
    ) {
        self.state.edit_topology(|state| {
            if let Some(target) = state.topology.workspaces.get_mut(workspace) {
                target.worktree_space = Some(membership);
            }
        });
    }
}

impl super::model::DaemonState {
    /// Admits terminal metadata for every pane that has none, as creation does.
    fn admit_seeded_terminals(&mut self) {
        let panes = self.topology.workspaces.iter().flat_map(|workspace| {
            workspace
                .tabs
                .iter()
                .flat_map(|tab| tab.panes.values())
                .map(|pane| {
                    (
                        pane.attached_terminal_id.clone(),
                        workspace.identity_cwd.clone(),
                    )
                })
        });
        for (terminal, cwd) in panes.collect::<Vec<_>>() {
            self.terminals
                .entry(terminal.clone())
                .or_insert_with(|| TerminalState::new(terminal, cwd));
        }
    }
}
