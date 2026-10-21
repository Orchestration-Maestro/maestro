//! Scheduled notification delivery using current process observations.

use super::{App, state::AgentNotificationDelivery};
use std::time::Instant;

/// Duration of a clipboard acknowledgement, independent of notification toast policy.
const COPY_FEEDBACK_DURATION: std::time::Duration = std::time::Duration::from_secs(2);

impl<C: FnMut() -> u128> App<C> {
    /// Records clipboard feedback without changing any agent notification.
    pub(crate) fn show_clipboard_feedback(&mut self) {
        if !self.state.toast_config.clipboard.enabled {
            self.state.copy_feedback = None;
            self.copy_feedback_deadline = None;
            return;
        }
        self.state.copy_feedback = Some(super::state::CopyFeedback {
            message: "copied to clipboard".into(),
        });
        self.copy_feedback_deadline = Some(Instant::now() + COPY_FEEDBACK_DURATION);
    }

    /// Removes due clipboard feedback once; the daemon scheduler consumes the change.
    pub fn expire_copy_feedback(&mut self, now: Instant) -> bool {
        if self
            .copy_feedback_deadline
            .is_none_or(|deadline| now < deadline)
        {
            return false;
        }
        self.copy_feedback_deadline = None;
        self.state.copy_feedback = None;
        true
    }

    /// Delivers due transitions with context reread from their live workspace roots.
    pub fn drain_due_agent_notifications(
        &mut self,
        now: Instant,
    ) -> Vec<AgentNotificationDelivery> {
        let mut deliveries = self.state.drain_due_agent_notifications(now);
        for delivery in &mut deliveries {
            let Some(workspace) = self
                .state
                .topology
                .workspaces
                .iter()
                .position(|workspace| workspace.id == delivery.workspace_id)
            else {
                continue;
            };
            let Some(context) = self.live_notification_context(workspace, delivery.pane_id) else {
                continue;
            };
            for toast in [&mut delivery.toast, &mut delivery.client_notification]
                .into_iter()
                .flatten()
            {
                toast.context.clone_from(&context);
            }
            if let Some(toast) = self.state.toast.as_mut()
                && toast.target.as_ref().is_some_and(|target| {
                    target.workspace_id == delivery.workspace_id
                        && target.pane_id == delivery.pane_id
                })
            {
                toast.context = context;
            }
        }
        deliveries
    }
}
