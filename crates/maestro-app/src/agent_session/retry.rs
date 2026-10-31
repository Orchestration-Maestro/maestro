//! Retrying a failed response with a visible, cancellable backoff.
use super::state::thrown;
use super::{AgentSession, AgentSessionEvent};
use crate::utils::lock::read;
use maestro_extensions::ExtensionExecution;
use maestro_models::{
    AssistantMessage, Cancellation, DiagnosticErrorInfo, StopReason, is_context_overflow,
};
use std::rc::Rc;
use std::sync::LazyLock;
use std::time::Duration;

/// Error texts a retry can recover from; a context overflow is recovered by compaction.
const RETRYABLE: &str = r"(?i)overloaded|provider.?returned.?error|rate.?limit|too many requests|429|500|502|503|504|service.?unavailable|server.?error|internal.?error|network.?error|connection.?error|connection.?refused|connection.?lost|websocket.?closed|websocket.?error|other side closed|fetch failed|upstream.?connect|reset before headers|socket hang up|ended without|http2 request did not get a response|timed? out|timeout|terminated|retry delay";

/// The compiled retryable classifier; absent only if the authored pattern is invalid,
/// which the recorded classification table rules out.
fn retryable() -> Option<&'static regex::Regex> {
    static PATTERN: LazyLock<Option<regex::Regex>> =
        LazyLock::new(|| regex::Regex::new(RETRYABLE).ok());
    PATTERN.as_ref()
}

impl<E: ExtensionExecution> AgentSession<E> {
    /// Whether `message` reports an error a retry can recover from.
    pub(crate) fn is_retryable_error(&self, message: &AssistantMessage) -> bool {
        if message.stop_reason != StopReason::Error {
            return false;
        }
        let Some(text) = message
            .error_message
            .as_deref()
            .filter(|text| !text.is_empty())
        else {
            return false;
        };
        let window = read(&self.model()).context_window;
        !is_context_overflow(message, Some(window))
            && retryable().is_some_and(|pattern| pattern.is_match(text))
    }

    /// Waits out the backoff and schedules one continuation, reporting whether it will run.
    /// Once started, the wait belongs to the session: dropping the returned future leaves it
    /// running, and `abort_retry` stops it.
    ///
    /// # Errors
    /// Returns the failure when the wait's task ended before the wait did, for example when
    /// the caller's task set was dropped mid-operation.
    pub(crate) async fn handle_retryable_error(
        self: &Rc<Self>,
        message: &AssistantMessage,
    ) -> Result<bool, DiagnosticErrorInfo> {
        let settings = self.settings_manager.borrow().get_retry_settings();
        if !settings.enabled {
            self.resolve_retry();
            return Ok(false);
        }
        self.create_retry_gate();
        let attempt = {
            let mut state = self.state.borrow_mut();
            state.retry_attempt = state.retry_attempt.saturating_add(1);
            state.retry_attempt
        };
        if f64::from(attempt) > settings.max_retries {
            let error = message.error_message.clone();
            self.emit(&AgentSessionEvent::AutoRetryEnd {
                success: false,
                attempt: attempt.saturating_sub(1),
                final_error: error,
            });
            self.state.borrow_mut().retry_attempt = 0;
            self.resolve_retry();
            return Ok(false);
        }
        let exponent = i32::try_from(attempt).unwrap_or(i32::MAX);
        let delay = settings.base_delay_ms * 2_f64.powi(exponent - 1);
        self.emit(&AgentSessionEvent::AutoRetryStart {
            attempt,
            max_attempts: settings.max_retries,
            delay_ms: delay,
            error_message: message
                .error_message
                .clone()
                .unwrap_or_else(|| "Unknown error".to_owned()),
        });
        self.drop_trailing_assistant(|_| true);
        let signal = Cancellation::new();
        self.state.borrow_mut().retry_signal = Some(signal.clone());
        let session = Rc::clone(self);
        self.start_operation(async move { session.wait_out_backoff(attempt, delay, &signal).await })
            .await
            .map_err(thrown)
    }

    /// Sleeps `delay` milliseconds under `signal`, then schedules the continuation; an abort
    /// ends the attempt instead.
    async fn wait_out_backoff(&self, attempt: u32, delay: f64, signal: &Cancellation) -> bool {
        if maestro_tools::process::sleep::sleep(millis(delay), Some(signal))
            .await
            .is_err()
        {
            self.cancel_retry(attempt);
            return false;
        }
        self.state.borrow_mut().retry_signal = None;
        self.schedule_continuation(Duration::ZERO);
        true
    }

    /// Stops a waiting retry without clearing the attempt it was making.
    pub fn abort_retry(&self) {
        let signal = self.state.borrow().retry_signal.clone();
        if let Some(signal) = signal {
            signal.abort();
        }
        self.resolve_retry();
    }

    /// Waits for a retry in flight and then for the agent to settle; returns at once when no
    /// retry is in flight.
    pub(crate) async fn wait_for_retry(&self) {
        let gate = self.state.borrow().retry_gate.clone();
        let Some(gate) = gate else {
            return;
        };
        drop(gate.await);
        self.agent.wait_for_idle().await;
    }

    /// Whether a response retry is in flight.
    #[must_use]
    pub fn is_retrying(&self) -> bool {
        self.state.borrow().retry_gate.is_some()
    }

    /// The attempts made by the retry in flight, or zero.
    #[must_use]
    pub fn retry_attempt(&self) -> u32 {
        self.state.borrow().retry_attempt
    }

    /// Whether a failed response is retried.
    #[must_use]
    pub fn auto_retry_enabled(&self) -> bool {
        self.settings_manager.borrow().get_retry_enabled()
    }

    /// Enables or disables retrying failed responses.
    ///
    /// # Errors
    /// Returns the publisher's refusal without changing preferences.
    pub fn set_auto_retry_enabled(&self, enabled: bool) -> Result<(), DiagnosticErrorInfo> {
        let edit = self.prepare_behaviour(&["retry", "enabled"], serde_json::json!(enabled))?;
        self.settings_manager.borrow_mut().apply_prepared_edit(edit);
        Ok(())
    }

    /// Creates the release every observer of this retry awaits, once.
    pub(super) fn create_retry_gate(&self) {
        let mut state = self.state.borrow_mut();
        if state.retry_gate.is_some() {
            return;
        }
        let (open, closed) = tokio::sync::oneshot::channel();
        state.retry_gate = Some(futures_util::future::FutureExt::shared(closed));
        state.retry_open = Some(open);
    }

    /// Ends a retry whose wait was aborted, clearing its attempts.
    fn cancel_retry(&self, attempt: u32) {
        {
            let mut state = self.state.borrow_mut();
            state.retry_signal = None;
            state.retry_attempt = 0;
        }
        self.emit(&AgentSessionEvent::AutoRetryEnd {
            success: false,
            attempt,
            final_error: Some("Retry cancelled".to_owned()),
        });
        self.resolve_retry();
    }

    /// Releases every observer of the retry in flight.
    pub(super) fn resolve_retry(&self) {
        let (gate, open) = {
            let mut state = self.state.borrow_mut();
            (state.retry_gate.take(), state.retry_open.take())
        };
        drop(gate);
        drop(open);
    }
}

/// The wait a JavaScript timer would arm for `delay`, where non-positive values arm none.
fn millis(delay: f64) -> Duration {
    if delay.is_finite() && delay > 0.0 {
        Duration::from_secs_f64(delay / 1000.0)
    } else {
        Duration::ZERO
    }
}
