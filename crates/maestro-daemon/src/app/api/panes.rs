//! Pane topology commands: list, current, get, focus, layout, process info and neighbours.

use super::{
    super::App,
    responses::{api_error, pane_not_found, pane_unresolved},
};
use maestro_process::{
    foreground::ForegroundProcess,
    pty::foreground::{self, ProcessDetectionMode},
};
use maestro_protocol::{
    rpc_types::RpcCommand,
    schema::{
        ErrorBody, PaneEdgesParams, PaneEdgesResult, PaneFocusDirectionParams,
        PaneFocusDirectionReason, PaneFocusDirectionResult, PaneLayoutSnapshot, PaneNeighborParams,
        PaneNeighborResult, PaneProcessInfo, PaneProcessInfoProcess, ResponseResult,
    },
    shared::Flag,
};
use maestro_workspace::layout::PaneId;

/// Where a resolved pane lives: its workspace, tab and identity.
#[derive(Clone, Copy)]
struct Located {
    /// Workspace display position.
    workspace: usize,
    /// Tab display position within the workspace.
    tab: usize,
    /// Pane identity allocated by the topology.
    pane: PaneId,
}

/// Copies one observed foreground member into the protocol's process record.
fn process_record(process: ForegroundProcess) -> PaneProcessInfoProcess {
    PaneProcessInfoProcess {
        pid: process.pid,
        cwd: foreground::process_cwd(process.pid).map(|cwd| cwd.display().to_string()),
        name: process.name,
        argv0: process.argv0,
        argv: process.argv,
        cmdline: process.cmdline,
    }
}

/// Reads the foreground group of `child` with each member's directory; the reads block.
fn observe_foreground(
    child: u32,
    mode: ProcessDetectionMode,
) -> Option<(u32, Vec<PaneProcessInfoProcess>)> {
    let job = foreground::foreground_job(child, mode)?;
    let processes = job.processes.into_iter().map(process_record).collect();
    Some((job.process_group_id, processes))
}

impl<C: FnMut() -> u128> App<C> {
    /// Routes pane commands to their handlers; commands of undelivered families fail.
    pub(super) async fn dispatch_pane_request(
        &mut self,
        command: RpcCommand,
    ) -> Result<ResponseResult, ErrorBody> {
        match command {
            RpcCommand::PaneList(params) => {
                self.handle_pane_list(params.workspace_id.as_deref()).await
            }
            RpcCommand::PaneCurrent(params) => {
                self.handle_pane_current(params.caller_pane_id.as_deref())
                    .await
            }
            RpcCommand::PaneGet(target) => self.handle_pane_get(&target.pane_id).await,
            RpcCommand::PaneFocus(target) => self.handle_pane_focus(&target.pane_id).await,
            RpcCommand::PaneLayout(params) => self.handle_pane_layout(params.pane_id.as_deref()),
            RpcCommand::PaneProcessInfo(params) => {
                self.handle_pane_process_info(params.pane_id.as_deref())
                    .await
            }
            RpcCommand::PaneNeighbor(params) => self.handle_pane_neighbor(&params),
            RpcCommand::PaneEdges(params) => self.handle_pane_edges(&params),
            RpcCommand::PaneFocusDirection(params) => self.handle_pane_focus_direction(&params),
            _ => Err(api_error("not_implemented", "method not implemented yet")),
        }
    }

    /// Lists the panes of one workspace, or of every workspace, in tab and split order.
    async fn handle_pane_list(
        &self,
        workspace_id: Option<&str>,
    ) -> Result<ResponseResult, ErrorBody> {
        let workspaces = match workspace_id {
            Some(id) => vec![self.target_workspace(id)?],
            None => (0..self.state.topology.workspaces.len()).collect(),
        };
        let mut panes = Vec::new();
        for workspace in workspaces {
            let order = self.state.topology.workspaces[workspace]
                .tabs
                .iter()
                .flat_map(|tab| tab.layout.pane_ids());
            for pane in order {
                panes.extend(self.pane_info(workspace, pane).await);
            }
        }
        Ok(ResponseResult::PaneList { panes })
    }

    /// Reads the caller's pane, or the focused pane when the caller names none.
    async fn handle_pane_current(&self, caller: Option<&str>) -> Result<ResponseResult, ErrorBody> {
        let (workspace, pane) = self.optional_pane(caller)?;
        self.pane_info(workspace, pane)
            .await
            .map(|pane| ResponseResult::PaneCurrent { pane })
            .ok_or_else(pane_unresolved)
    }

    /// Reads one pane.
    async fn handle_pane_get(&self, id: &str) -> Result<ResponseResult, ErrorBody> {
        let (workspace, pane) = self.target_pane(id)?;
        self.pane_info_result(workspace, pane, id).await
    }

    /// Focuses one pane, activating its tab and workspace and seeing its tab's panes.
    async fn handle_pane_focus(&mut self, id: &str) -> Result<ResponseResult, ErrorBody> {
        let (workspace, pane) = self.target_pane(id)?;
        self.state.focus_pane_in_workspace(workspace, pane);
        self.state.mark_active_tab_seen();
        self.state.settle_terminal_mode_after_focus();
        self.pane_info_result(workspace, pane, id).await
    }

    /// Projects the tab of the named or focused pane.
    fn handle_pane_layout(&self, pane_id: Option<&str>) -> Result<ResponseResult, ErrorBody> {
        let at = self.located_pane(pane_id)?;
        Ok(ResponseResult::PaneLayout {
            layout: self.tab_layout(at)?,
        })
    }

    /// Copies the pane's process facts into the protocol record.
    async fn handle_pane_process_info(
        &self,
        pane_id: Option<&str>,
    ) -> Result<ResponseResult, ErrorBody> {
        let (workspace, pane) = self.optional_pane(pane_id)?;
        let runtime = self
            .state
            .topology
            .workspaces
            .get(workspace)
            .and_then(|target| target.terminal_id(pane))
            .and_then(|terminal| self.terminal_runtimes.get(terminal))
            .ok_or_else(pane_unresolved)?;
        let observed = runtime
            .observe_processes(observe_foreground)
            .await
            .map_err(|error| {
                api_error(
                    "internal_error",
                    format!("process observation failed: {error}"),
                )
            })?
            .flatten();
        let (group, processes) = observed.map_or((None, Vec::new()), |(id, list)| (Some(id), list));
        Ok(ResponseResult::PaneProcessInfo {
            process_info: PaneProcessInfo {
                pane_id: self.public_pane(workspace, pane)?,
                shell_pid: runtime.child_pid(),
                foreground_process_group_id: group,
                tty: None,
                foreground_processes: processes,
            },
        })
    }

    /// Names the pane next to the source in a direction, if any.
    fn handle_pane_neighbor(
        &self,
        params: &PaneNeighborParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let at = self.located_pane(params.pane_id.as_deref())?;
        let neighbor_pane_id = self
            .directional_pane_target(at.workspace, at.tab, at.pane, params.direction)
            .and_then(|pane| self.state.public_pane_id(at.workspace, pane));
        Ok(ResponseResult::PaneNeighbor {
            neighbor: PaneNeighborResult {
                pane_id: self.public_pane(at.workspace, at.pane)?,
                direction: params.direction,
                neighbor_pane_id,
                layout: self.tab_layout(at)?,
            },
        })
    }

    /// Tells which sides of the view area the pane's cell touches.
    fn handle_pane_edges(&self, params: &PaneEdgesParams) -> Result<ResponseResult, ErrorBody> {
        let at = self.located_pane(params.pane_id.as_deref())?;
        let area = self.state.view_area();
        let pane_id = self.public_pane(at.workspace, at.pane)?;
        let cell = self.state.topology.workspaces[at.workspace].tabs[at.tab]
            .layout
            .panes(area)
            .into_iter()
            .find(|info| info.id == at.pane)
            .ok_or_else(|| pane_not_found(&pane_id))?
            .rect;
        Ok(ResponseResult::PaneEdges {
            edges: PaneEdgesResult {
                pane_id,
                left: cell.x <= area.x,
                right: cell.right() >= area.right(),
                up: cell.y <= area.y,
                down: Flag(cell.bottom() >= area.bottom()),
                layout: self.tab_layout(at)?,
            },
        })
    }

    /// Moves focus to the neighbour in a direction, reporting when there is none.
    fn handle_pane_focus_direction(
        &mut self,
        params: &PaneFocusDirectionParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let at = self.located_pane(params.pane_id.as_deref())?;
        let source_pane_id = self.public_pane(at.workspace, at.pane)?;
        let target = self.directional_pane_target(at.workspace, at.tab, at.pane, params.direction);
        if let Some(target) = target {
            self.state.focus_pane_in_workspace(at.workspace, target);
            self.state.switch_workspace_tab(at.workspace, at.tab);
            self.state.settle_terminal_mode_after_focus();
        }
        let focused = self.state.topology.workspaces[at.workspace].tabs[at.tab]
            .layout
            .focused();
        Ok(ResponseResult::PaneFocusDirection {
            focus: PaneFocusDirectionResult {
                changed: target.is_some(),
                reason: target
                    .is_none()
                    .then_some(PaneFocusDirectionReason::NoNeighbor),
                source_pane_id,
                focused_pane_id: self.state.public_pane_id(at.workspace, focused),
                layout: self.tab_layout(at)?,
            },
        })
    }

    /// Resolves an explicit pane target to its workspace and pane.
    fn target_pane(&self, id: &str) -> Result<(usize, PaneId), ErrorBody> {
        self.state
            .parse_pane_id(id)
            .ok_or_else(|| pane_not_found(id))
    }

    /// Resolves a pane named by `pane_id`, or the focused pane of the active workspace.
    fn optional_pane(&self, pane_id: Option<&str>) -> Result<(usize, PaneId), ErrorBody> {
        match pane_id {
            Some(id) => self.state.parse_pane_id(id),
            None => self.state.active.and_then(|workspace| {
                let pane = self
                    .state
                    .topology
                    .workspaces
                    .get(workspace)?
                    .focused_pane_id()?;
                Some((workspace, pane))
            }),
        }
        .ok_or_else(pane_unresolved)
    }

    /// Resolves an optional pane target together with the tab that holds it.
    fn located_pane(&self, pane_id: Option<&str>) -> Result<Located, ErrorBody> {
        let (workspace, pane) = self.optional_pane(pane_id)?;
        let tab = self.state.topology.workspaces[workspace]
            .find_tab_index_for_pane(pane)
            .ok_or_else(|| {
                pane_not_found(
                    &self
                        .state
                        .public_pane_id(workspace, pane)
                        .unwrap_or_default(),
                )
            })?;
        Ok(Located {
            workspace,
            tab,
            pane,
        })
    }

    /// The stable public handle of a live pane.
    fn public_pane(&self, workspace: usize, pane: PaneId) -> Result<String, ErrorBody> {
        self.state
            .public_pane_id(workspace, pane)
            .ok_or_else(pane_unresolved)
    }

    /// The layout of the located pane's tab.
    fn tab_layout(&self, at: Located) -> Result<PaneLayoutSnapshot, ErrorBody> {
        self.pane_layout_snapshot(at.workspace, at.tab)
            .ok_or_else(|| api_error("pane_layout_unavailable", "pane layout unavailable"))
    }

    /// The record of a live pane as a command result.
    async fn pane_info_result(
        &self,
        workspace: usize,
        pane: PaneId,
        id: &str,
    ) -> Result<ResponseResult, ErrorBody> {
        self.pane_info(workspace, pane)
            .await
            .map(|pane| ResponseResult::PaneInfo { pane })
            .ok_or_else(|| pane_not_found(id))
    }
}
