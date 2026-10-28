//! The `notification.show` command under the daemon's notification delivery policy.

use super::super::{
    App,
    state::{ToastKind, ToastNotification},
};
use crate::settings::model::ToastDelivery;
use maestro_protocol::schema::{
    ErrorBody, NotificationShowParams, NotificationShowReason, ResponseResult,
};
use std::time::{Duration, Instant};

/// Shortest interval between two delivered API notifications.
const API_NOTIFICATION_RATE_LIMIT: Duration = Duration::from_secs(1);

/// Longest notification title, in Unicode scalars.
const TITLE_MAX_CHARS: usize = 80;

/// Longest notification body, in Unicode scalars.
const BODY_MAX_CHARS: usize = 240;

impl<C: FnMut() -> u128> App<C> {
    /// Shows a caller's notification as a workspace toast when that delivery is configured.
    ///
    /// A retained toast reports `busy` and a delivery less than one second after the last
    /// one reports `rate_limited`. Terminal and system delivery need a foreground client,
    /// which the daemon does not have, so they report `no_foreground_client`.
    ///
    /// # Errors
    /// Returns `invalid_params` when the cleaned title is empty.
    pub(super) fn handle_notification_show(
        &mut self,
        params: &NotificationShowParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let Some(title) = sanitized_notification_text(&params.title, TITLE_MAX_CHARS) else {
            return Err(super::responses::api_error(
                "invalid_params",
                "notification title is empty",
            ));
        };
        let now = Instant::now();
        let reason = match self.state.toast_config.delivery {
            ToastDelivery::Off => NotificationShowReason::Disabled,
            ToastDelivery::Workspace if self.state.toast.is_some() => NotificationShowReason::Busy,
            _ if self.api_notification_rate_limited(now) => NotificationShowReason::RateLimited,
            ToastDelivery::Terminal | ToastDelivery::System => {
                NotificationShowReason::NoForegroundClient
            }
            ToastDelivery::Workspace => {
                self.last_api_notification_at = Some(now);
                let body = params
                    .body
                    .as_deref()
                    .and_then(|body| sanitized_notification_text(body, BODY_MAX_CHARS));
                self.state.toast = Some(ToastNotification {
                    kind: ToastKind::UpdateInstalled,
                    title,
                    context: body.unwrap_or_default(),
                    position: params.position,
                    target: None,
                });
                self.sync_toast_deadline(None);
                NotificationShowReason::Shown
            }
        };
        Ok(ResponseResult::NotificationShow {
            shown: reason == NotificationShowReason::Shown,
            reason,
        })
    }

    /// Whether the last delivered API notification is less than one second old.
    fn api_notification_rate_limited(&self, now: Instant) -> bool {
        self.last_api_notification_at
            .is_some_and(|last| now.duration_since(last) < API_NOTIFICATION_RATE_LIMIT)
    }
}

/// Cleans notification text: CR, LF and tab become spaces, other control characters go,
/// whitespace runs become one space, and at most `max_chars` scalars remain before the
/// final trim; nothing left means no text.
fn sanitized_notification_text(value: &str, max_chars: usize) -> Option<String> {
    let mut sanitized = String::new();
    let mut count = 0;
    let mut previous_space = false;
    for ch in value.chars() {
        let ch = match ch {
            '\n' | '\r' | '\t' => ' ',
            ch if ch.is_control() => continue,
            ch => ch,
        };
        if ch.is_whitespace() {
            if previous_space {
                continue;
            }
            previous_space = true;
            sanitized.push(' ');
        } else {
            previous_space = false;
            sanitized.push(ch);
        }
        count += 1;
        if count >= max_chars {
            break;
        }
    }
    let sanitized = sanitized.trim();
    (!sanitized.is_empty()).then(|| sanitized.to_owned())
}
