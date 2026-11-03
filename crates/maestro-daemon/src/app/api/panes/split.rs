//! `pane.split`: splits a pane and starts a process in the new one.

use super::super::{
    env::normalize_launch_env,
    responses::{api_error, pane_unresolved},
};
use crate::app::{
    App,
    creation::{RootLaunch, spawn_root},
};
use maestro_protocol::{
    events::EventData,
    schema::{ErrorBody, PaneRightClickTarget, PaneSplitParams, ResponseResult},
};
use maestro_workspace::layout::{PaneId, SplitRequest};
use std::{io, path::PathBuf};

impl<C: FnMut() -> u128> App<C> {
    /// Splits the named pane, the focused pane of the named workspace or the focused pane of
    /// the active workspace, and answers with the new pane once its process runs.
    ///
    /// # Errors
    /// Returns `pane_not_found` without a target, `invalid_env` for a bad launch environment
    /// and `pane_split_failed` when the process cannot start; no pane is added then.
    pub(super) async fn handle_pane_split(
        &mut self,
        params: PaneSplitParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let (workspace, target) = self.split_target(&params)?;
        let extra_env = normalize_launch_env(params.env)?;
        let cwd = match params.cwd {
            Some(cwd) => PathBuf::from(cwd),
            None => self.new_pane_cwd(workspace, target).await,
        };
        let previous_focus = self.state.current_pane_focus_target();
        let request = SplitRequest {
            target,
            direction: params.direction,
            ratio: params.ratio,
            focus_new_pane: params.focus,
        };
        let root = RootLaunch {
            cwd,
            extra_env,
            argv: None,
        };
        let pane = self
            .spawn_split(workspace, request, root)
            .ok_or_else(pane_unresolved)?
            .map_err(|error| api_error("pane_split_failed", error.to_string()))?;
        let located = self.locate(workspace, pane).ok_or_else(pane_unresolved)?;
        if let Some(state) = self.state.topology.workspaces[workspace].pane_state_mut(pane) {
            state.right_click_passthrough = params.right_click == PaneRightClickTarget::Pane;
        }
        if params.focus {
            self.state.switch_workspace_tab(workspace, located.tab);
            self.state
                .record_pane_focus_change(previous_focus, workspace, pane);
            self.state.settle_terminal_mode_after_focus();
        }
        self.state.session_dirty = true;
        let info = self
            .pane_info(workspace, pane)
            .await
            .ok_or_else(pane_unresolved)?;
        self.event_hub
            .push(EventData::PaneCreated { pane: info.clone() });
        self.emit_layout_updated_event(workspace, located.tab);
        Ok(ResponseResult::PaneInfo { pane: info })
    }

    /// Resolves the pane to split from its handle, else the focused pane of the named or
    /// the active workspace.
    fn split_target(&self, params: &PaneSplitParams) -> Result<(usize, PaneId), ErrorBody> {
        match (&params.target_pane_id, &params.workspace_id) {
            (Some(id), _) => self.state.parse_pane_id(id),
            (None, Some(id)) => self
                .state
                .parse_workspace_id(id)
                .and_then(|workspace| self.focused_pane(workspace)),
            (None, None) => self
                .state
                .active
                .and_then(|workspace| self.focused_pane(workspace)),
        }
        .ok_or_else(pane_unresolved)
    }

    /// Splits `request.target` in a workspace and starts a process in the new pane as `root`
    /// says; `None` when the target is gone.
    pub(in crate::app::api) fn spawn_split(
        &mut self,
        workspace: usize,
        request: SplitRequest,
        root: RootLaunch,
    ) -> Option<io::Result<PaneId>> {
        let size = self.state.estimate_pane_size();
        let launch = &self.launch;
        let config = &self.config;
        let spawned =
            self.state
                .topology
                .split_pane_with_runtime(workspace, request, |pane, spawn| {
                    spawn_root(
                        launch.runtime_config(config, pane, size),
                        config,
                        root,
                        spawn,
                    )
                })?;
        Some(spawned.map(|(pane, (terminal, runtime))| {
            self.terminal_runtimes.insert(terminal.id.clone(), runtime);
            self.state.terminals.insert(terminal.id.clone(), terminal);
            self.state.remove_alias_shadowed_by_new_pane(pane);
            pane
        }))
    }
}
