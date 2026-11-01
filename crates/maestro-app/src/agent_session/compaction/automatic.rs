//! Compaction the session decides on after a response.
use super::super::state::{message_records, thrown};
use super::super::{AgentSession, AgentSessionEvent, CompactionReason};
use super::BeforeCompaction;
use crate::utils::lock::read;
use indexmap::IndexMap;
use maestro_extensions::ExtensionExecution;
use maestro_models::{
    AssistantMessage, Cancellation, DiagnosticErrorInfo, StopReason, is_context_overflow,
};
use maestro_session::CompactionResult;
use maestro_session::compaction::{
    CompactionSettings, calculate_context_tokens, estimate_context_tokens, should_compact,
};
use maestro_session::session_manager::{ReadonlySessionManager, get_latest_compaction_entry};
use std::rc::Rc;

impl<E: ExtensionExecution> AgentSession<E> {
    /// Compacts when the finished response overflowed or passed the reserve, reporting whether
    /// the run continues: after an overflow recovery, or when queued messages wait. A started
    /// compaction belongs to the session: dropping the returned future leaves it running.
    ///
    /// `skip_aborted_check` keeps an aborted response out of the check unless the caller
    /// checks before a prompt.
    ///
    /// # Errors
    /// Returns a failure reading the stored branch or the response's usage, or the failure
    /// when the compaction's task ended before it did; a failing compaction reports its
    /// failure through `compaction_end` instead.
    pub(crate) async fn check_compaction(
        self: &Rc<Self>,
        message: &AssistantMessage,
        skip_aborted_check: bool,
    ) -> Result<bool, DiagnosticErrorInfo> {
        if !self.settings_manager.borrow().get_compaction_enabled() {
            return Ok(false);
        }
        if skip_aborted_check && message.stop_reason == StopReason::Aborted {
            return Ok(false);
        }
        let model = self.model();
        let window = read(&model).context_window;
        let same_model = {
            let model = read(&model);
            message.provider == model.provider && message.model == model.id
        };
        let boundary = self.compaction_boundary()?;
        if Self::started_before_boundary(message.timestamp, boundary.as_deref()) {
            return Ok(false);
        }
        if same_model && is_context_overflow(message, Some(window)) {
            if self.state.borrow().overflow_recovery_attempted {
                self.emit(&AgentSessionEvent::CompactionEnd {
                    reason: CompactionReason::Overflow,
                    result: None,
                    aborted: false,
                    will_retry: false,
                    error_message: Some(
                        "Context overflow recovery failed after one compact-and-retry attempt. \
                         Try reducing context or switching to a larger-context model."
                            .to_owned(),
                    ),
                });
                return Ok(false);
            }
            self.state.borrow_mut().overflow_recovery_attempted = true;
            self.drop_trailing_assistant(|_| true);
            return self
                .run_auto_compaction(CompactionReason::Overflow, true)
                .await;
        }
        let tokens = self.context_tokens(message, boundary.as_deref())?;
        let Some(tokens) = tokens else {
            return Ok(false);
        };
        if should_compact(tokens, window, &self.compaction_policy()) {
            return self
                .run_auto_compaction(CompactionReason::Threshold, false)
                .await;
        }
        Ok(false)
    }

    /// Starts one automatic compaction on the session and waits for it to end, reporting
    /// whether the run continues.
    ///
    /// # Errors
    /// Returns the failure when the compaction's task ended before it did.
    async fn run_auto_compaction(
        self: &Rc<Self>,
        reason: CompactionReason,
        will_retry: bool,
    ) -> Result<bool, DiagnosticErrorInfo> {
        let session = Rc::clone(self);
        let run = async move { session.auto_compaction(reason, will_retry).await };
        self.start_operation(run).await.map_err(thrown)
    }

    /// Runs one automatic compaction, reporting every outcome through `compaction_end` and
    /// whether the run continues: after a recovery, or when queued messages wait.
    async fn auto_compaction(&self, reason: CompactionReason, will_retry: bool) -> bool {
        let settings = self.compaction_policy();
        self.emit(&AgentSessionEvent::CompactionStart { reason });
        let signal = Cancellation::new();
        self.state.borrow_mut().auto_compaction_signal = Some(signal.clone());
        let outcome = self
            .auto_compaction_content(&settings, &signal)
            .await
            .unwrap_or_else(|error| Outcome::Failed {
                message: error.message,
            });
        let compacted = matches!(outcome, Outcome::Compacted { .. });
        let end = match outcome {
            Outcome::Skipped { aborted } => AgentSessionEvent::CompactionEnd {
                reason,
                result: None,
                aborted,
                will_retry: false,
                error_message: None,
            },
            Outcome::Failed { message } => AgentSessionEvent::CompactionEnd {
                reason,
                result: None,
                aborted: false,
                will_retry: false,
                error_message: Some(if reason == CompactionReason::Overflow {
                    format!("Context overflow recovery failed: {message}")
                } else {
                    format!("Auto-compaction failed: {message}")
                }),
            },
            Outcome::Compacted { result } => AgentSessionEvent::CompactionEnd {
                reason,
                result: Some(result),
                aborted: false,
                will_retry,
                error_message: None,
            },
        };
        self.close_compaction(&end);
        if !compacted {
            return false;
        }
        if will_retry {
            self.drop_trailing_assistant(|message| message.stop_reason == StopReason::Error);
            return true;
        }
        self.agent.has_queued_messages()
    }

    /// Clears the one-recovery-per-turn mark after a response succeeds.
    pub(crate) fn clear_overflow_recovery_attempt(&self) {
        self.state.borrow_mut().overflow_recovery_attempted = false;
    }

    /// The automatic compaction body over the policy `settings` captured before its start.
    async fn auto_compaction_content(
        &self,
        settings: &CompactionSettings,
        signal: &Cancellation,
    ) -> Result<Outcome, DiagnosticErrorInfo> {
        let Some(auth) = self.configured_auth().await else {
            return Ok(Outcome::Skipped { aborted: false });
        };
        let prepared = self.prepared_branch(settings)?;
        let Some(preparation) = prepared.preparation else {
            return Ok(Outcome::Skipped { aborted: false });
        };
        let supplied = self
            .before_compaction(preparation, prepared.entries, None, signal)
            .await?;
        let (result, from_extension) = match supplied {
            BeforeCompaction::Cancelled => return Ok(Outcome::Skipped { aborted: true }),
            BeforeCompaction::Supplied { result } => (result, true),
            BeforeCompaction::Continue { preparation } => (
                self.summarize(&preparation, auth, None, signal).await?,
                false,
            ),
        };
        if signal.is_aborted() {
            return Ok(Outcome::Skipped { aborted: true });
        }
        let entry = self.store_compaction(&result, from_extension)?;
        self.announce_compaction(entry, from_extension).await?;
        Ok(Outcome::Compacted { result })
    }

    /// The key, headers and endpoint automatic compaction uses, absent without a key.
    async fn configured_auth(
        &self,
    ) -> Option<(String, Option<IndexMap<String, String>>, Option<String>)> {
        let model = self.model();
        let auth = self.model_registry.get_api_key_and_headers(&model).await;
        let maestro_catalog::ResolvedRequestAuth::Ok {
            api_key: Some(api_key),
            headers,
            base_url,
        } = auth
        else {
            return None;
        };
        (!api_key.is_empty()).then_some((api_key, headers, base_url))
    }

    /// The context tokens the finished response reports, or the last real usage's estimate when it
    /// failed or reported none; absent when nothing is known.
    fn context_tokens(
        &self,
        message: &AssistantMessage,
        boundary: Option<&str>,
    ) -> Result<Option<f64>, DiagnosticErrorInfo> {
        if message.stop_reason != StopReason::Error {
            let usage = serde_json::to_value(&message.usage).map_err(thrown)?;
            let direct = calculate_context_tokens(&usage).map_err(thrown)?;
            if direct > 0.0 {
                return Ok(Some(direct));
            }
        }
        let records = message_records(&read(&self.messages()))?;
        let estimate = estimate_context_tokens(&records).map_err(thrown)?;
        let Some(index) = estimate.last_usage_index else {
            return Ok(None);
        };
        let Some(usage) = records.get(index) else {
            return Ok(None);
        };
        let is_assistant =
            usage.get("role").and_then(serde_json::Value::as_str) == Some("assistant");
        if is_assistant {
            let timestamp = usage
                .get("timestamp")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(f64::NAN);
            if Self::started_before_boundary(timestamp, boundary) {
                return Ok(None);
            }
        }
        Ok(Some(estimate.tokens))
    }

    /// The creation time of the newest compaction entry on the branch.
    fn compaction_boundary(&self) -> Result<Option<String>, DiagnosticErrorInfo> {
        let manager = self.session_manager.borrow();
        let branch = manager.get_branch(None).map_err(thrown)?;
        let boundary = get_latest_compaction_entry(&branch)
            .map_err(thrown)?
            .and_then(|entry| entry.get("timestamp"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        Ok(boundary)
    }

    /// Whether `timestamp` is at or before the latest compaction boundary.
    fn started_before_boundary(timestamp: f64, boundary: Option<&str>) -> bool {
        boundary
            .and_then(maestro_session::messages::parse_timestamp_millis)
            .and_then(num_traits::cast::cast::<i64, f64>)
            .is_some_and(|boundary| timestamp <= boundary)
    }
}

/// What one automatic compaction produced.
enum Outcome {
    /// Nothing to store; the event reports whether a handler refused it.
    Skipped {
        /// Whether a handler refused it.
        aborted: bool,
    },
    /// The summarizer failed.
    Failed {
        /// The failure message.
        message: String,
    },
    /// A compaction was stored.
    Compacted {
        /// The stored compaction.
        result: CompactionResult,
    },
}
