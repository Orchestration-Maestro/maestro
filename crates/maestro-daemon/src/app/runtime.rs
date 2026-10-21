//! Scheduled notification delivery using current process observations.

use super::{App, state::AgentNotificationDelivery};
use std::time::Instant;

impl<C: FnMut() -> u128> App<C> {
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
