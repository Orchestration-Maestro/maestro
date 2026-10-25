//! Scheduled notification delivery using current process observations.

mod cadence;
mod run;
mod scheduled;

use super::{App, state::AgentNotificationDelivery};
use std::time::Instant;

/// Duration of a clipboard acknowledgement, independent of notification toast policy.
const COPY_FEEDBACK_DURATION: std::time::Duration = std::time::Duration::from_secs(2);

/// Maximum internal observations processed in one responsive loop pass.
const APP_EVENT_DRAIN_LIMIT: usize = 64;

impl<C: FnMut() -> u128> App<C> {
    /// Drains one bounded internal-event batch without starving the next loop operation.
    pub fn drain_internal_events(&mut self) -> bool {
        self.drain_internal_events_up_to(APP_EVENT_DRAIN_LIMIT).1
    }

    /// Drains all queued observations when correctness requires an exhaustive boundary.
    pub fn drain_all_internal_events(&mut self) -> bool {
        let mut changed = false;
        loop {
            let (had_event, batch_changed) =
                self.drain_internal_events_up_to(APP_EVENT_DRAIN_LIMIT);
            changed |= batch_changed;
            if !had_event {
                return changed;
            }
        }
    }

    /// Reports occupancy separately from visible changes, including bell-only batches.
    fn drain_internal_events_up_to(&mut self, limit: usize) -> (bool, bool) {
        let mut changed = false;
        let mut had_event = false;
        for _ in 0..limit {
            let Ok(event) = self.event_rx.try_recv() else {
                break;
            };
            had_event = true;
            changed |= self.handle_internal_event_with_render_impact(event);
        }
        (had_event, changed)
    }

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
            if self.state.toast_config.delivery == crate::settings::model::ToastDelivery::Workspace
                && delivery.toast.is_some()
                && let Some(toast) = self.state.toast.as_mut()
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
