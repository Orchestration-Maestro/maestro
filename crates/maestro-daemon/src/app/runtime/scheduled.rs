//! Due work consumed by the daemon's single loop.

use super::super::{
    App,
    state::{ToastKind, ToastNotification},
};
use crate::paths::DaemonPaths;
use std::time::{Duration, Instant};

impl<C: FnMut() -> u128> App<C> {
    /// Consumes due current-capability deadlines without creating another timer or loop.
    pub async fn handle_scheduled_tasks(&mut self, now: Instant, paths: &DaemonPaths) -> bool {
        self.finish_history_reload(paths);
        let mut changed = self.expire_copy_feedback(now);
        if self
            .config_diagnostic_deadline
            .is_some_and(|deadline| now >= deadline)
        {
            self.config_diagnostic_deadline = None;
            self.state.config_diagnostic = None;
            changed = true;
        }
        if self.toast_deadline.is_some_and(|deadline| now >= deadline) {
            self.toast_deadline = None;
            self.state.toast = None;
            changed = true;
        }
        if self
            .state
            .next_pending_agent_notification_deadline()
            .is_some_and(|deadline| now >= deadline)
        {
            let previous = self.state.toast.clone();
            changed |= !self.drain_due_agent_notifications(now).is_empty();
            self.sync_toast_deadline(previous.as_ref());
        }
        if self
            .state
            .next_managed_agent_deadline()
            .is_some_and(|deadline| now >= deadline)
        {
            let panes = self.state.reconcile_managed_agents_at(now);
            changed |= !panes.is_empty();
            for (workspace, pane) in panes {
                self.emit_pane_updated(workspace, pane).await;
            }
            self.sync_session_save_schedule();
        }
        if self
            .session_save_deadline
            .is_some_and(|deadline| now >= deadline)
        {
            self.start_background_session_save(paths);
        }
        changed |= self.expire_due_metadata(now).await;
        changed | self.handle_tab_bar_status_tasks(now)
    }

    /// Consumes due metadata on the loop's existing timer and publishes each changed record.
    async fn expire_due_metadata(&mut self, now: Instant) -> bool {
        let Some(deadline) = self
            .state
            .next_agent_metadata_expiry()
            .filter(|deadline| now >= *deadline)
        else {
            return false;
        };
        let previous = self.state.toast.clone();
        for update in self.state.expire_agent_metadata_at(deadline, now) {
            self.refresh_new_workspace_toast_context_for_update(&update, previous.as_ref());
            self.emit_pane_state_update(&update);
        }
        for (workspace, pane) in self.state.expire_pane_metadata_tokens(now) {
            self.emit_pane_updated(workspace, pane).await;
        }
        for index in self.state.expire_workspace_metadata_tokens(now) {
            self.emit_workspace_metadata_updated(index);
        }
        true
    }

    /// Advances toast lifetime only when the retained notification itself changes.
    pub(crate) fn sync_toast_deadline(&mut self, previous: Option<&ToastNotification>) {
        if self.state.toast.as_ref() != previous {
            self.toast_deadline = self.state.toast.as_ref().map(|toast| {
                let duration = match toast.kind {
                    ToastKind::NeedsAttention => Duration::from_secs(8),
                    ToastKind::Finished => Duration::from_secs(5),
                    ToastKind::UpdateInstalled => Duration::from_secs(3),
                };
                Instant::now() + duration
            });
        }
    }
}
