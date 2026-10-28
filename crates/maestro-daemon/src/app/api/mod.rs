//! Internal observations applied through daemon state and live process context, and the
//! dispatch of workspace commands to their handlers.

mod env;
mod manifests;
mod notifications;
mod overlay;
mod respawn;
mod responses;
mod state_events;
mod tabs;
mod workspaces;
mod worktrees;

use super::{
    App,
    actions::{PaneStateUpdate, notification_context},
    state::ToastNotification,
};
use crate::{
    api::RequestMessage, events::AppEvent, paths::DaemonPaths, settings::model::ToastDelivery,
};
use maestro_protocol::{
    rpc_types::RpcCommand,
    schema::{ErrorBody, ResponseResult},
};
use maestro_settings::SettingsManager;
use responses::api_error;
pub(super) use worktrees::WorktreeOperations;

impl<C: FnMut() -> u128> App<C> {
    /// Answers one connection request through its response channel.
    ///
    /// A worktree create or remove waits for Git: the loop moves on and the answer follows
    /// from [`Self::settle_worktree_operations`]. Every other command is answered at once.
    pub async fn answer_request(
        &mut self,
        request: RequestMessage,
        settings: &mut SettingsManager,
        paths: &DaemonPaths,
    ) {
        let RequestMessage {
            command,
            respond_to,
            ..
        } = request;
        self.begin_request(&command).await;
        match command {
            RpcCommand::WorktreeCreate(params) => self.start_worktree_create(params, respond_to),
            RpcCommand::WorktreeRemove(params) => {
                self.start_worktree_remove(params, respond_to).await;
            }
            command => {
                let response = self.route_request(command, settings, paths).await;
                let _ = respond_to.send(response);
            }
        }
    }

    /// Answers one workspace command after applying every queued internal observation.
    ///
    /// A command that changes what a view shows marks render work. `settings` and `paths`
    /// serve the configuration and detection-catalog commands.
    ///
    /// # Errors
    /// Returns the command's failure: `not_implemented` for a command without a handler.
    pub async fn handle_api_request(
        &mut self,
        command: RpcCommand,
        settings: &mut SettingsManager,
        paths: &DaemonPaths,
    ) -> Result<ResponseResult, ErrorBody> {
        self.begin_request(&command).await;
        self.route_request(command, settings, paths).await
    }

    /// Applies every queued observation before a command is answered and marks the render
    /// work a command that changes what a view shows will cause.
    async fn begin_request(&mut self, command: &RpcCommand) {
        self.drain_all_internal_events();
        self.sync_pending_terminal_titles().await;
        if crate::api::request_changes_ui(command) {
            self.launch.render_dirty.request_generic();
        }
    }

    /// Sends a command to its handler.
    async fn route_request(
        &mut self,
        command: RpcCommand,
        settings: &mut SettingsManager,
        paths: &DaemonPaths,
    ) -> Result<ResponseResult, ErrorBody> {
        match command {
            RpcCommand::ServerStop(_) => {
                self.state.should_quit.0 = true;
                Ok(ResponseResult::Ok {})
            }
            RpcCommand::ServerReloadConfig(_) => {
                let agent_dir = std::path::Path::new(paths.agent_dir());
                let (report, errors) = self.reload_config(settings, agent_dir).await;
                for error in errors {
                    tracing::warn!(%error.error, "failed to read workspace preferences");
                }
                Ok(ResponseResult::ConfigReload {
                    status: report.status,
                    diagnostics: report.diagnostics,
                })
            }
            RpcCommand::ServerAgentManifests(_) => Ok(self.agent_manifest_status(paths)),
            RpcCommand::ServerReloadAgentManifests(_) => self.reload_agent_manifests(paths),
            RpcCommand::NotificationShow(params) => self.handle_notification_show(&params),
            command => self.dispatch_topology_request(command).await,
        }
    }

    /// Routes workspace and tab commands to their handlers; unhandled families return an error.
    async fn dispatch_topology_request(
        &mut self,
        command: RpcCommand,
    ) -> Result<ResponseResult, ErrorBody> {
        match command {
            RpcCommand::WorkspaceList(_) => Ok(self.handle_workspace_list()),
            RpcCommand::WorkspaceGet(target) => self.handle_workspace_get(&target.workspace_id),
            RpcCommand::WorkspaceCreate(params) => self.handle_workspace_create(params).await,
            RpcCommand::WorkspaceFocus(target) => self.handle_workspace_focus(&target.workspace_id),
            RpcCommand::WorkspaceRename(params) => self.handle_workspace_rename(params),
            RpcCommand::WorkspaceMove(params) => self.handle_workspace_move(&params),
            RpcCommand::WorkspaceMoveBlock(params) => self.handle_workspace_move_block(&params),
            RpcCommand::WorkspaceReportMetadata(params) => {
                self.handle_workspace_report_metadata(params)
            }
            RpcCommand::WorkspaceClose(target) => {
                self.handle_workspace_close(&target.workspace_id).await
            }
            RpcCommand::WorktreeList(params) => self.handle_worktree_list(&params),
            RpcCommand::WorktreeOpen(params) => self.handle_worktree_open(params).await,
            RpcCommand::WorktreeCreate(_) => Err(api_error(
                "invalid_request",
                "worktree.create is handled asynchronously by the app runtime",
            )),
            RpcCommand::WorktreeRemove(_) => Err(api_error(
                "invalid_request",
                "worktree.remove is handled asynchronously by the app runtime",
            )),
            RpcCommand::TabList(params) => self.handle_tab_list(params.workspace_id.as_deref()),
            RpcCommand::TabGet(target) => self.handle_tab_get(&target.tab_id),
            RpcCommand::TabCreate(params) => self.handle_tab_create(params).await,
            RpcCommand::TabFocus(target) => self.handle_tab_focus(&target.tab_id),
            RpcCommand::TabRename(params) => self.handle_tab_rename(params),
            RpcCommand::TabMove(params) => self.handle_tab_move(&params),
            RpcCommand::TabClose(target) => self.handle_tab_close(&target.tab_id).await,
            _ => Err(api_error("not_implemented", "method not implemented yet")),
        }
    }

    /// Applies a background observation through the retained state transition owner.
    pub fn handle_internal_event(&mut self, event: AppEvent) -> Vec<PaneStateUpdate> {
        self.apply_internal_event(event).0
    }

    /// Applies one observation and reports its visible render impact.
    pub fn handle_internal_event_with_render_impact(&mut self, event: AppEvent) -> bool {
        self.apply_internal_event(event).1
    }

    /// Single dispatch owner shared by state callers and the bounded daemon drain.
    fn apply_internal_event(&mut self, event: AppEvent) -> (Vec<PaneStateUpdate>, bool) {
        let previous = self.state.toast.clone();
        let result = self.dispatch_internal_event(event);
        self.sync_toast_deadline(previous.as_ref());
        result
    }

    /// Applies an observation that changes no pane state and reports its render impact; any
    /// other observation is handed back.
    fn apply_standalone_event(&mut self, event: AppEvent) -> Result<bool, AppEvent> {
        match event {
            AppEvent::TabBarCommandFinished {
                generation,
                segment_index,
                result,
            } => {
                let changed =
                    self.handle_tab_bar_command_finished(generation, segment_index, result);
                if changed {
                    self.launch.render_dirty.request_generic();
                    self.launch.render_notify.notify_one();
                }
                Ok(changed)
            }
            AppEvent::WorktreeOperationFinished {
                operation_id,
                result,
            } => {
                self.worktree_operations.finish(operation_id, result);
                Ok(false)
            }
            AppEvent::ClipboardWrite { .. } => {
                self.show_clipboard_feedback();
                self.launch.render_dirty.request_generic();
                self.launch.render_notify.notify_one();
                Ok(true)
            }
            event => Err(event),
        }
    }

    /// Dispatches a background observation through its existing domain owner.
    fn dispatch_internal_event(&mut self, event: AppEvent) -> (Vec<PaneStateUpdate>, bool) {
        let event = match self.apply_standalone_event(event) {
            Ok(changed) => return (Vec::new(), changed),
            Err(event) => event,
        };
        let changed = !matches!(&event, AppEvent::TerminalBell { .. });
        self.publish_pane_process_exit(&event);
        if self.respawn_shell_after_exit(&event) {
            return (Vec::new(), true);
        }
        self.emit_pane_exit(&event);
        let overlay_exit = self.take_overlay_before_exit(&event);
        let previous_toast = self.state.toast.clone();
        let manifest_update_agents =
            if let AppEvent::AgentDetectionManifestsUpdated { updated, .. } = &event {
                Some(updated.iter().map(|item| item.agent).collect::<Vec<_>>())
            } else {
                None
            };
        let released = match &event {
            AppEvent::HookAgentReleased {
                pane_id,
                known_agent,
                ..
            } => known_agent.map(|agent| (*pane_id, agent)),
            _ => None,
        };
        let updates = self.state.handle_app_event(event);
        self.sync_runtime_detection_controls(released, &updates);
        if let Some(agents) = manifest_update_agents {
            self.reset_agent_detection_for_agents(&agents);
        }
        for update in &updates {
            self.refresh_new_workspace_toast_context_for_update(update, previous_toast.as_ref());
            self.emit_pane_state_update(update);
        }
        if let Some(exit) = overlay_exit {
            self.restore_overlay_after_exit(exit);
        }
        (updates, changed)
    }

    /// Refreshes matching retained detectors after their shared manifests have changed.
    fn reset_agent_detection_for_agents(&self, agents: &[maestro_detection::agents::Agent]) {
        if agents.is_empty() {
            return;
        }
        for (terminal_id, terminal) in &self.state.terminals {
            let Some(agent) = terminal.effective_known_agent().or(terminal.detected_agent) else {
                continue;
            };
            if agents.contains(&agent)
                && let Some(runtime) = self.terminal_runtimes.get(terminal_id)
            {
                runtime.reset_agent_detection();
            }
        }
    }

    /// Resolves a workspace's label from its first root process before stale metadata.
    pub(super) fn live_notification_context(
        &self,
        workspace: usize,
        pane: maestro_workspace::layout::PaneId,
    ) -> Option<String> {
        let target = self.state.topology.workspaces.get(workspace)?;
        let label = crate::workspace::display_name_from(
            target,
            &self.state.terminals,
            &self.terminal_runtimes,
            None,
        );
        Some(notification_context(target, &label, workspace, pane))
    }

    /// Refreshes only a newly produced workspace toast for this exact pane identity.
    pub(in crate::app) fn refresh_new_workspace_toast_context_for_update(
        &mut self,
        update: &PaneStateUpdate,
        previous_toast: Option<&ToastNotification>,
    ) {
        if self.state.toast_config.delivery != ToastDelivery::Workspace
            || self.state.toast.as_ref() == previous_toast
        {
            return;
        }
        let Some(target) = self
            .state
            .toast
            .as_ref()
            .and_then(|toast| toast.target.as_ref())
        else {
            return;
        };
        if target.pane_id != update.pane_id
            || self
                .state
                .topology
                .workspaces
                .get(update.ws_idx)
                .is_none_or(|workspace| workspace.id != target.workspace_id)
        {
            return;
        }
        if let Some(context) = self.live_notification_context(update.ws_idx, update.pane_id)
            && let Some(toast) = self.state.toast.as_mut()
        {
            toast.context = context;
        }
    }
}
