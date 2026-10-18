//! One owned allocator across all workspaces.
use super::{Workspace, encode_public_number};
use crate::{
    layout::{PaneId, SplitRequest},
    tab::{SpawnRequest, Tab},
    terminal_id::TerminalId,
};
use std::{
    io,
    path::{Path, PathBuf},
};

/// Counter domains shared by one topology owner.
struct IdAllocator {
    /// Next numeric pane identity.
    pane: u32,
    /// Next terminal counter.
    terminal: u64,
    /// Next workspace number.
    workspace: usize,
}
/// Workspace collection with one allocator and a caller-supplied microsecond clock.
pub struct WorkspaceTopology<C> {
    /// Workspaces in display order.
    pub workspaces: Vec<Workspace>,
    /// Identity domains shared across that collection.
    allocator: IdAllocator,
    /// Clock evaluated when allocating a terminal identity.
    clock: C,
}
impl<C: FnMut() -> u128> WorkspaceTopology<C> {
    /// Create an independent topology with a supplied terminal-identity clock.
    #[must_use]
    pub fn new(clock: C) -> Self {
        Self {
            workspaces: Vec::new(),
            allocator: IdAllocator {
                pane: 1,
                terminal: 1,
                workspace: 1,
            },
            clock,
        }
    }
    /// Allocate a pane identity shared by this owner's workspaces and tabs.
    pub fn alloc_pane_id(&mut self) -> PaneId {
        let id = PaneId::from_raw(self.allocator.pane);
        self.allocator.pane += 1;
        id
    }
    /// Allocate a fresh terminal identity using the supplied clock.
    pub fn alloc_terminal_id(&mut self) -> TerminalId {
        let id = TerminalId::from_parts((self.clock)(), self.allocator.terminal);
        self.allocator.terminal += 1;
        id
    }
    /// Allocate a readable workspace handle without resetting on removal.
    pub fn generate_workspace_id(&mut self) -> String {
        let number = self.allocator.workspace;
        self.allocator.workspace += 1;
        format!("w{}", encode_public_number(number))
    }
    /// Create a workspace and return its initial spawn result to the caller.
    ///
    /// # Errors
    /// Propagates the caller's spawn error without publishing a workspace.
    pub fn new_with_tab<T>(
        &mut self,
        cwd: PathBuf,
        home: Option<&Path>,
        spawn: impl FnOnce(PaneId, SpawnRequest) -> io::Result<T>,
    ) -> io::Result<(PaneId, T)> {
        let id = self.generate_workspace_id();
        let pane = self.alloc_pane_id();
        let request = SpawnRequest {
            workspace_id: id.clone(),
            tab_number: 1,
            pane_number: 1,
            terminal_id: self.alloc_terminal_id(),
        };
        let (tab, result) = Tab::new_with_runtime(pane, request, spawn)?;
        let label = super::git::fallback_label_from_cwd(&cwd, home);
        self.workspaces.push(Workspace::new(id, cwd, label, tab));
        Ok((pane, result))
    }
    /// Create a tab, preserving its reserved public number even if spawn fails.
    pub fn create_tab_with_runtime<T>(
        &mut self,
        workspace: usize,
        spawn: impl FnOnce(PaneId, SpawnRequest) -> io::Result<T>,
    ) -> Option<io::Result<(PaneId, T)>> {
        let ws = self.workspaces.get_mut(workspace)?;
        let number = ws.next_public_tab_number;
        ws.next_public_tab_number += 1;
        let pane_number = ws.next_public_pane_number;
        let id = ws.id.clone();
        let pane = self.alloc_pane_id();
        let request = SpawnRequest {
            workspace_id: id,
            tab_number: number,
            pane_number,
            terminal_id: self.alloc_terminal_id(),
        };
        Some(
            Tab::new_with_runtime(pane, request, spawn).map(|(tab, result)| {
                let ws = &mut self.workspaces[workspace];
                ws.register_new_pane_with_number(pane, pane_number);
                ws.tabs.push(tab);
                (pane, result)
            }),
        )
    }
    /// Split a live pane, leaving runtime ownership with the caller.
    pub fn split_pane_with_runtime<T>(
        &mut self,
        workspace: usize,
        request: SplitRequest,
        spawn: impl FnOnce(PaneId, SpawnRequest) -> io::Result<T>,
    ) -> Option<io::Result<(PaneId, T)>> {
        let ws = self.workspaces.get(workspace)?;
        let index = ws
            .tabs
            .iter()
            .position(|tab| tab.panes.contains_key(&request.target))?;
        let pane_number = ws.next_public_pane_number;
        let id = ws.id.clone();
        let tab_number = ws.tabs[index].number;
        let pane = self.alloc_pane_id();
        let spawn_request = SpawnRequest {
            workspace_id: id,
            tab_number,
            pane_number,
            terminal_id: self.alloc_terminal_id(),
        };
        let ws = &mut self.workspaces[workspace];
        Some(
            ws.tabs[index]
                .split_pane_with_runtime(pane, request, spawn_request, spawn)
                .map(|(pane, result)| {
                    ws.register_new_pane_with_number(pane, pane_number);
                    (pane, result)
                }),
        )
    }

    /// Admit a saved workspace, reseeding numeric domains and replacing terminal identities.
    pub fn admit_restored_workspace(&mut self, mut workspace: Workspace) -> usize {
        for tab in &mut workspace.tabs {
            workspace.next_public_tab_number = workspace.next_public_tab_number.max(tab.number + 1);
            for (pane, state) in &mut tab.panes {
                self.allocator.pane = self.allocator.pane.max(pane.raw() + 1);
                state.attached_terminal_id = self.alloc_terminal_id();
            }
        }
        self.reserve_workspace_ids(std::slice::from_ref(&workspace));
        for number in workspace.public_pane_numbers.values() {
            workspace.next_public_pane_number = workspace.next_public_pane_number.max(number + 1);
        }
        self.workspaces.push(workspace);
        self.workspaces.len() - 1
    }

    /// Recover transferred state in a newly numbered workspace without respawning.
    pub fn from_existing_pane(
        owner: &mut Self,
        cwd: PathBuf,
        home: Option<&Path>,
        moved: crate::tab::MovedPane,
    ) -> usize {
        owner.allocator.pane = owner.allocator.pane.max(moved.pane_id.raw() + 1);
        let id = owner.generate_workspace_id();
        let label = super::git::fallback_label_from_cwd(&cwd, home);
        let tab = Tab::from_existing_pane(1, None, moved);
        owner.workspaces.push(Workspace::new(id, cwd, label, tab));
        owner.workspaces.len() - 1
    }

    /// Reserve handles above a supplied set of restored workspace identities.
    pub fn reserve_workspace_ids(&mut self, workspaces: &[Workspace]) {
        let next = workspaces
            .iter()
            .filter_map(|workspace| super::public_workspace_number(&workspace.id))
            .max()
            .and_then(|number| number.checked_add(1));
        if let Some(next) = next {
            self.allocator.workspace = self.allocator.workspace.max(next);
        }
    }
}
