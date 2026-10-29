//! Internal observations applied through daemon state and live process context.

mod overlay;
mod respawn;
mod state_events;

use super::{
    App,
    actions::{PaneStateUpdate, notification_context},
    state::ToastNotification,
};
use crate::{events::AppEvent, settings::model::ToastDelivery};

impl<C: FnMut() -> u128> App<C> {
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

    /// Dispatches a background observation through its existing domain owner.
    fn dispatch_internal_event(&mut self, event: AppEvent) -> (Vec<PaneStateUpdate>, bool) {
        let event = match self.dispatch_render_only_event(event) {
            Ok(changed) => return (Vec::new(), changed),
            Err(event) => event,
        };
        if matches!(event, AppEvent::TerminalCwdReported { .. }) {
            self.request_git_identity_refresh(std::time::Instant::now());
        }
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

    /// Applies an observation whose only effect is a render request, or hands it back.
    fn dispatch_render_only_event(&mut self, event: AppEvent) -> Result<bool, AppEvent> {
        let changed = match event {
            AppEvent::TabBarCommandFinished {
                generation,
                segment_index,
                result,
            } => self.handle_tab_bar_command_finished(generation, segment_index, result),
            AppEvent::ClipboardWrite { .. } => {
                self.show_clipboard_feedback();
                true
            }
            AppEvent::GitStatusRefreshed {
                results,
                cache_updates,
            } => self.handle_git_status_refreshed(results, cache_updates),
            other => return Err(other),
        };
        if changed {
            self.launch.render_dirty.request_generic();
            self.launch.render_notify.notify_one();
        }
        Ok(changed)
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
