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
        if let AppEvent::TabBarCommandFinished {
            generation,
            segment_index,
            result,
        } = event
        {
            let changed = self.handle_tab_bar_command_finished(generation, segment_index, result);
            if changed {
                self.launch.render_dirty.request_generic();
                self.launch.render_notify.notify_one();
            }
            return (Vec::new(), changed);
        }
        if let AppEvent::ClipboardWrite { .. } = event {
            self.show_clipboard_feedback();
            self.launch.render_dirty.request_generic();
            self.launch.render_notify.notify_one();
            return (Vec::new(), true);
        }
        let changed = !matches!(&event, AppEvent::TerminalBell { .. });
        self.publish_pane_process_exit(&event);
        if self.respawn_shell_after_exit(&event) {
            return (Vec::new(), true);
        }
        self.emit_pane_exit(&event);
        let overlay_exit = self.take_overlay_before_exit(&event);
        let previous_toast = self.state.toast.clone();
        let updates = self.state.handle_app_event(event);
        for update in &updates {
            self.refresh_new_workspace_toast_context_for_update(update, previous_toast.as_ref());
            self.emit_pane_state_update(update);
        }
        if let Some(exit) = overlay_exit {
            self.restore_overlay_after_exit(exit);
        }
        (updates, changed)
    }

    /// Resolves a workspace's label from its first root process before stale metadata.
    pub(super) fn live_notification_context(
        &self,
        workspace: usize,
        pane: maestro_workspace::layout::PaneId,
    ) -> Option<String> {
        let target = self.state.topology.workspaces.get(workspace)?;
        let terminal = target
            .tabs
            .first()
            .and_then(|tab| tab.terminal_id(tab.root_pane));
        let cwd = terminal
            .and_then(|terminal| self.terminal_runtimes.get(terminal))
            .and_then(crate::pane::runtime::PaneRuntime::cwd)
            .or_else(|| {
                terminal
                    .and_then(|terminal| self.state.terminals.get(terminal))
                    .map(|terminal| terminal.cwd.clone())
            })
            .unwrap_or_else(|| target.identity_cwd.clone());
        let label = target.display_name_for_cwd(&cwd, None);
        Some(notification_context(target, &label, workspace, pane))
    }

    /// Refreshes only a newly produced workspace toast for this exact pane identity.
    fn refresh_new_workspace_toast_context_for_update(
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
