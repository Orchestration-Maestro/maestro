//! Shared deadline selection and independent rendering and presentation cadence.

use super::super::App;
use std::time::{Duration, Instant};

/// Smallest interval between attempts on either rendering clock.
const MIN_RENDER_INTERVAL: Duration = Duration::from_millis(16);

impl<C: FnMut() -> u128> App<C> {
    /// Includes terminal-size polling only for an interactive caller.
    #[must_use]
    pub fn next_loop_deadline(&self, now: Instant, needs_render: bool) -> Option<Instant> {
        [
            Some(self.next_resize_poll),
            self.next_headless_loop_deadline(now, needs_render),
        ]
        .into_iter()
        .flatten()
        .min()
    }

    /// Selects one deadline from the producers currently owned by the daemon.
    #[must_use]
    pub fn next_headless_loop_deadline(&self, now: Instant, needs_render: bool) -> Option<Instant> {
        let render = needs_render
            .then_some(self.last_render_at)
            .flatten()
            .map(|last| last + MIN_RENDER_INTERVAL)
            .filter(|deadline| *deadline > now);
        [
            self.config_diagnostic_deadline,
            self.toast_deadline,
            self.state.next_pending_agent_notification_deadline(),
            self.state.next_managed_agent_deadline(),
            self.state.next_agent_metadata_expiry(),
            self.copy_feedback_deadline,
            self.session_save_deadline,
            self.next_tab_bar_status_deadline(),
            self.git_refresh_deadline(),
            render,
        ]
        .into_iter()
        .flatten()
        .min()
    }

    /// Whether a new render attempt fits the rendering cadence.
    #[must_use]
    pub fn can_render_now(&self, now: Instant) -> bool {
        self.last_render_at
            .is_none_or(|last| now.saturating_duration_since(last) >= MIN_RENDER_INTERVAL)
    }

    /// Whether an actual presentation fits its independent cadence.
    #[must_use]
    pub fn can_present_now(&self, now: Instant) -> bool {
        self.last_presentation_at
            .is_none_or(|last| now.saturating_duration_since(last) >= MIN_RENDER_INTERVAL)
    }

    /// Records an attempt without advancing presentation time for hidden frames.
    pub fn record_render_attempt(&mut self, now: Instant, presentation: bool) {
        self.last_render_at = Some(now);
        if presentation {
            self.last_presentation_at = Some(now);
        }
    }
}
