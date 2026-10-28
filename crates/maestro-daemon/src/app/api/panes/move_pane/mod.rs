//! `pane.move`: moves a live pane, with its terminal, into another tab, a new tab or a new
//! workspace.

mod destination;

use super::{super::responses::api_error, Located};
use crate::app::{App, actions::PaneMoveRecovery};
use destination::{Destination, Resolution};
use maestro_protocol::{
    events::EventData,
    schema::{ErrorBody, PaneMoveParams, PaneMoveReason, PaneMoveResult, ResponseResult},
};
use maestro_workspace::{
    layout::{PaneId, SplitRequest},
    tab::MovedPane,
    terminal_id::TerminalId,
};

/// A pane about to move, with the handles and state of the place it leaves.
struct Source {
    /// Where the pane lives now.
    at: Located,
    /// Terminal attached to the pane.
    terminal: TerminalId,
    /// Handle of the pane before the move.
    pane_id: String,
    /// Handle of the workspace before the move.
    workspace_id: String,
    /// Handle of the tab before the move.
    tab_id: String,
    /// What puts the pane back if the destination refuses it.
    recovery: PaneMoveRecovery,
}

/// What a move created around the pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Created {
    /// The pane went into an existing tab.
    Nothing,
    /// The pane became a new tab of an existing workspace.
    Tab,
    /// The pane became the only tab of a new workspace.
    Workspace,
}

/// Where a pane landed and what its move created and closed.
struct Placed {
    /// Display position of the receiving workspace.
    workspace: usize,
    /// Display position of the receiving tab.
    tab: usize,
    /// The moved pane.
    pane: PaneId,
    /// What the move created.
    created: Created,
    /// Whether the source tab went away.
    closed_tab: bool,
    /// Whether the source workspace went away.
    closed_workspace: bool,
}

impl Placed {
    /// A pane placed in a tab that creates nothing, with nothing closed yet.
    fn in_tab(workspace: usize, tab: usize, pane: PaneId, created: Created) -> Self {
        Self {
            workspace,
            tab,
            pane,
            created,
            closed_tab: false,
            closed_workspace: false,
        }
    }
}

impl<C: FnMut() -> u128> App<C> {
    /// Moves a pane beside a pane of another tab, to a new tab or to a new workspace.
    ///
    /// The pane keeps its terminal. A pane that leaves its workspace gets a handle there and
    /// keeps answering to the old one. A move that cannot happen is answered with its reason;
    /// one the destination refuses after the pane was taken puts the pane back.
    ///
    /// # Errors
    /// Returns `pane_not_found`, `tab_not_found`, `workspace_not_found`, `target_pane_not_found`
    /// or `pane_move_failed`.
    pub(super) async fn handle_pane_move(
        &mut self,
        params: PaneMoveParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let source = self.move_source(&params.pane_id)?;
        let destination = match self.resolve_move_destination(&source, params.destination)? {
            Resolution::Unchanged(reason, tab) => {
                return self.unchanged_move(&source, reason, tab).await;
            }
            Resolution::Move(destination) => destination,
        };
        let placed = self.relocate_pane(&source, destination, params.focus)?;
        self.publish_move(&source, placed).await
    }

    /// Captures the pane to move and the place it leaves.
    fn move_source(&self, id: &str) -> Result<Source, ErrorBody> {
        let missing = || api_error("pane_not_found", "source pane not found");
        let at = self
            .state
            .parse_pane_id(id)
            .and_then(|(workspace, pane)| self.locate(workspace, pane))
            .ok_or_else(missing)?;
        let workspace = &self.state.topology.workspaces[at.workspace];
        let terminal = workspace
            .terminal_id(at.pane)
            .cloned()
            .ok_or_else(missing)?;
        let tab_id = self
            .state
            .public_tab_id(at.workspace, at.tab)
            .ok_or_else(|| api_error("tab_not_found", "source tab not found"))?;
        Ok(Source {
            at,
            terminal,
            pane_id: self
                .state
                .public_pane_id(at.workspace, at.pane)
                .unwrap_or_else(|| id.to_owned()),
            workspace_id: workspace.id.clone(),
            tab_id,
            recovery: PaneMoveRecovery {
                workspace: at.workspace,
                workspace_id: workspace.id.clone(),
                workspace_label: workspace.custom_name.clone(),
                tab_label: workspace.tabs[at.tab].custom_name.clone(),
                worktree_space: workspace.worktree_space.clone(),
                identity_cwd: workspace.identity_cwd.clone(),
            },
        })
    }

    /// Answers a move that changes nothing, describing the source tab and the one it refers to.
    async fn unchanged_move(
        &self,
        source: &Source,
        reason: PaneMoveReason,
        tab: (usize, usize),
    ) -> Result<ResponseResult, ErrorBody> {
        let source_layout = self.layout_at(source.at.workspace, source.at.tab)?;
        let target_layout = if tab == (source.at.workspace, source.at.tab) {
            source_layout.clone()
        } else {
            self.layout_at(tab.0, tab.1)?
        };
        let pane = self
            .pane_info(source.at.workspace, source.at.pane)
            .await
            .ok_or_else(|| api_error("pane_not_found", "source pane not found"))?;
        Ok(ResponseResult::PaneMove {
            move_result: PaneMoveResult {
                changed: false,
                reason: Some(reason),
                previous_pane_id: source.pane_id.clone(),
                previous_workspace_id: source.workspace_id.clone(),
                previous_tab_id: source.tab_id.clone(),
                pane: Box::new(pane),
                source_layout: Some(Box::new(source_layout)),
                focused_pane_id: target_layout.focused_pane_id.clone(),
                target_layout: Box::new(target_layout),
                created_workspace: None,
                created_tab: None,
                closed_workspace_id: None,
                closed_tab_id: None,
            },
        })
    }

    /// Takes the pane out of its source and places it at the destination, restoring the source
    /// when the destination refuses it.
    fn relocate_pane(
        &mut self,
        source: &Source,
        destination: Destination,
        focus: bool,
    ) -> Result<Placed, ErrorBody> {
        let previous_focus = self.state.current_pane_focus_target();
        let taken = self.state.topology.workspaces[source.at.workspace]
            .take_pane_for_move(source.at.pane)
            .ok_or_else(|| api_error("pane_move_failed", "source pane could not be moved"))?;
        let leaves = destination.leaves_workspace(&source.workspace_id);
        if leaves {
            self.state.topology.workspaces[source.at.workspace]
                .unregister_moved_pane(source.at.pane);
            self.state
                .public_pane_id_aliases
                .insert(source.pane_id.clone(), source.at.pane);
        }
        let closed_workspace = taken.workspace_empty && leaves;
        if closed_workspace {
            self.state.remove_emptied_workspace(source.at.workspace);
        }
        let mut placed = self.attach_moved_pane(source, destination, taken.moved, focus)?;
        placed.closed_tab = taken.removed_tab_idx.is_some();
        placed.closed_workspace = closed_workspace;
        if focus || self.state.active.is_none() {
            self.state
                .switch_workspace_tab(placed.workspace, placed.tab);
            self.state
                .record_pane_focus_change(previous_focus, placed.workspace, placed.pane);
            self.state.settle_terminal_mode_after_focus();
        }
        Ok(placed)
    }

    /// Places a taken pane at the destination.
    fn attach_moved_pane(
        &mut self,
        source: &Source,
        destination: Destination,
        moved: MovedPane,
        focus: bool,
    ) -> Result<Placed, ErrorBody> {
        match destination {
            Destination::ExistingTab {
                tab_id,
                target,
                split,
                ratio,
                ..
            } => {
                let request = SplitRequest {
                    target,
                    direction: split,
                    ratio: Some(ratio),
                    focus_new_pane: focus,
                };
                self.attach_beside(source, &tab_id, moved, &request)
            }
            Destination::NewTab {
                workspace_id,
                label,
            } => {
                let Ok(workspace) = self.target_workspace(&workspace_id) else {
                    return Err(self.refuse_move(source, moved, "target workspace disappeared"));
                };
                let pane = moved.pane_id;
                let tab = self.state.topology.workspaces[workspace]
                    .create_tab_from_existing_pane(moved, label);
                Ok(Placed::in_tab(workspace, tab, pane, Created::Tab))
            }
            Destination::NewWorkspace { label, tab_label } => {
                let cwd = self
                    .state
                    .terminals
                    .get(&source.terminal)
                    .map(|terminal| terminal.cwd.clone())
                    .or_else(|| std::env::current_dir().ok())
                    .unwrap_or_else(|| "/".into());
                let pane = moved.pane_id;
                let workspace = self
                    .state
                    .new_workspace_from_moved_pane(cwd, label, tab_label, moved);
                Ok(Placed::in_tab(workspace, 0, pane, Created::Workspace))
            }
        }
    }

    /// Places a taken pane beside `request.target` in the tab with the handle `tab_id`.
    fn attach_beside(
        &mut self,
        source: &Source,
        tab_id: &str,
        moved: MovedPane,
        request: &SplitRequest,
    ) -> Result<Placed, ErrorBody> {
        let Some((workspace, tab)) = self.state.parse_tab_id(tab_id) else {
            return Err(self.refuse_move(source, moved, "target tab disappeared"));
        };
        match self.state.topology.workspaces[workspace]
            .insert_moved_pane_into_tab(tab, moved, request)
        {
            Ok(pane) => Ok(Placed::in_tab(workspace, tab, pane, Created::Nothing)),
            Err(moved) => Err(self.refuse_move(source, moved, "target pane could not be split")),
        }
    }

    /// Puts a refused pane back where it came from and describes the failure.
    fn refuse_move(&mut self, source: &Source, moved: MovedPane, message: &str) -> ErrorBody {
        self.state
            .recover_failed_pane_move(source.recovery.clone(), moved);
        api_error("pane_move_failed", message)
    }

    /// Publishes the move and answers with its record.
    async fn publish_move(
        &mut self,
        source: &Source,
        placed: Placed,
    ) -> Result<ResponseResult, ErrorBody> {
        let Placed {
            workspace,
            tab,
            pane,
            ..
        } = placed;
        let created_workspace = (placed.created == Created::Workspace)
            .then(|| self.workspace_info(workspace))
            .flatten();
        let created_tab = (placed.created != Created::Nothing)
            .then(|| self.tab_info(workspace, tab))
            .flatten();
        self.state.remove_alias_shadowed_by_new_pane(pane);
        self.state.session_dirty = true;
        let info = self
            .pane_info(workspace, pane)
            .await
            .ok_or_else(|| api_error("pane_move_failed", "moved pane is unavailable"))?;
        let source_layout = if placed.closed_workspace {
            None
        } else {
            self.state
                .parse_tab_id(&source.tab_id)
                .and_then(|(workspace, tab)| self.pane_layout_snapshot(workspace, tab))
        };
        let target_layout = self.layout_at(workspace, tab)?;
        let move_result = PaneMoveResult {
            changed: true,
            reason: None,
            previous_pane_id: source.pane_id.clone(),
            previous_workspace_id: source.workspace_id.clone(),
            previous_tab_id: source.tab_id.clone(),
            pane: Box::new(info),
            source_layout: source_layout.map(Box::new),
            focused_pane_id: target_layout.focused_pane_id.clone(),
            target_layout: Box::new(target_layout),
            created_workspace,
            created_tab,
            closed_workspace_id: placed.closed_workspace.then(|| source.workspace_id.clone()),
            closed_tab_id: placed.closed_tab.then(|| source.tab_id.clone()),
        };
        self.emit_move_events(&move_result);
        Ok(ResponseResult::PaneMove { move_result })
    }

    /// Publishes what a move closed, created and moved, then the layouts it changed.
    fn emit_move_events(&self, moved: &PaneMoveResult) {
        if let Some(tab_id) = &moved.closed_tab_id {
            self.event_hub.push(EventData::TabClosed {
                tab_id: tab_id.clone(),
                workspace_id: moved.previous_workspace_id.clone(),
            });
        }
        if let Some(workspace_id) = &moved.closed_workspace_id {
            self.event_hub.push(EventData::WorkspaceClosed {
                workspace_id: workspace_id.clone(),
                workspace: None,
            });
        }
        if let Some(workspace) = &moved.created_workspace {
            self.event_hub.push(EventData::WorkspaceCreated {
                workspace: workspace.clone(),
            });
        }
        if let Some(tab) = &moved.created_tab {
            self.event_hub
                .push(EventData::TabCreated { tab: tab.clone() });
        }
        self.event_hub.push(EventData::PaneMoved {
            previous_pane_id: moved.previous_pane_id.clone(),
            previous_workspace_id: moved.previous_workspace_id.clone(),
            previous_tab_id: moved.previous_tab_id.clone(),
            pane: moved.pane.clone(),
            created_workspace: moved.created_workspace.clone(),
            created_tab: moved.created_tab.clone(),
            closed_workspace_id: moved.closed_workspace_id.clone(),
            closed_tab_id: moved.closed_tab_id.clone(),
        });
        if let Some(layout) = &moved.source_layout {
            self.emit_layout_updated_snapshot((**layout).clone());
        }
        self.emit_layout_updated_snapshot((*moved.target_layout).clone());
    }
}
