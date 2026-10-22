//! Compaction the session decides on after a response.
use super::super::{AgentSession, AgentSessionEvent, CompactionReason};
use super::BeforeCompaction;
use crate::utils::lock::read;
use indexmap::IndexMap;
use maestro_extensions::ExtensionExecution;
use maestro_models::{
    AssistantMessage, Cancellation, DiagnosticErrorInfo, StopReason, is_context_overflow,
};
use maestro_session::CompactionResult;
use maestro_session::compaction::SummaryOptions;
use maestro_session::compaction::{
    calculate_context_tokens, estimate_context_tokens, should_compact,
};
use maestro_session::session_manager::{ReadonlySessionManager, get_latest_compaction_entry};
use std::time::Duration;

impl<E: ExtensionExecution> AgentSession<E> {
    /// Compacts when the finished response overflowed or passed the reserve.
    ///
    /// `skip_aborted_check` keeps an aborted response out of the check unless the caller
    /// checks before a prompt.
    ///
    /// # Errors
    /// Returns a storing, handler or listener failure.
    pub async fn check_compaction(
        &self,
        message: &AssistantMessage,
        skip_aborted_check: bool,
    ) -> Result<(), DiagnosticErrorInfo> {
        if !self.settings_manager.borrow().get_compaction_enabled() {
            return Ok(());
        }
        if skip_aborted_check && message.stop_reason == StopReason::Aborted {
            return Ok(());
        }
        let model = self.model();
        let window = read(&model).context_window;
        let same_model = {
            let model = read(&model);
            message.provider == model.provider && message.model == model.id
        };
        let boundary = self.compaction_boundary()?;
        if Self::started_before_boundary(message.timestamp, boundary.as_deref()) {
            return Ok(());
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
                return Ok(());
            }
            self.state.borrow_mut().overflow_recovery_attempted = true;
            self.drop_trailing_assistant();
            self.run_auto_compaction(CompactionReason::Overflow, true)
                .await?;
            return Ok(());
        }
        let tokens = self.context_tokens(message, boundary.as_deref())?;
        let Some(tokens) = tokens else {
            return Ok(());
        };
        if should_compact(tokens, window, &self.compaction_policy()) {
            self.run_auto_compaction(CompactionReason::Threshold, false)
                .await?;
        }
        Ok(())
    }

    /// Runs one automatic compaction, reporting every outcome through the session events.
    ///
    /// # Errors
    /// Returns a listener failure raised while announcing an outcome.
    pub(super) async fn run_auto_compaction(
        &self,
        reason: CompactionReason,
        will_retry: bool,
    ) -> Result<(), DiagnosticErrorInfo> {
        self.emit(&AgentSessionEvent::CompactionStart { reason });
        let signal = Cancellation::new();
        self.state.borrow_mut().auto_compaction_signal = Some(signal.clone());
        let outcome = self.auto_compaction_content(&signal).await;
        self.state.borrow_mut().auto_compaction_signal = None;
        match outcome {
            Ok(Outcome::Skipped { aborted }) => {
                self.emit(&AgentSessionEvent::CompactionEnd {
                    reason,
                    result: None,
                    aborted,
                    will_retry: false,
                    error_message: None,
                });
            }
            Ok(Outcome::Failed { message }) => {
                self.emit(&AgentSessionEvent::CompactionEnd {
                    reason,
                    result: None,
                    aborted: false,
                    will_retry: false,
                    error_message: Some(if reason == CompactionReason::Overflow {
                        format!("Context overflow recovery failed: {message}")
                    } else {
                        format!("Auto-compaction failed: {message}")
                    }),
                });
            }
            Ok(Outcome::Compacted { result }) => {
                self.emit(&AgentSessionEvent::CompactionEnd {
                    reason,
                    result: Some(result),
                    aborted: false,
                    will_retry,
                    error_message: None,
                });
                if will_retry {
                    self.drop_trailing_assistant();
                    self.schedule_continuation(Duration::from_millis(100));
                } else if self.agent.has_queued_messages() {
                    self.schedule_continuation(Duration::from_millis(100));
                }
            }
            Err(error) => return Err(error),
        }
        Ok(())
    }

    /// Clears the one-recovery-per-turn mark after a response succeeds.
    pub fn clear_overflow_recovery_attempt(&self) {
        self.state.borrow_mut().overflow_recovery_attempted = false;
    }

    /// The automatic compaction body, reporting the outcomes a caller announces.
    async fn auto_compaction_content(
        &self,
        signal: &Cancellation,
    ) -> Result<Outcome, DiagnosticErrorInfo> {
        let Some((api_key, headers)) = self.configured_auth().await else {
            return Ok(Outcome::Skipped { aborted: false });
        };
        let prepared = self.prepared_branch().map_err(failed)?;
        let Some(preparation) = prepared.preparation else {
            return Ok(Outcome::Skipped { aborted: false });
        };
        let supplied = self
            .before_compaction(&preparation, prepared.entries.clone(), None, signal)
            .await
            .map_err(failed)?;
        let (result, from_extension) = match supplied {
            BeforeCompaction::Cancelled => return Ok(Outcome::Skipped { aborted: true }),
            BeforeCompaction::Supplied { result } => (result, true),
            BeforeCompaction::Continue => {
                let options = SummaryOptions {
                    api_key,
                    headers,
                    signal: Some(signal.clone()),
                    custom_instructions: None,
                    thinking_level: Some(self.thinking_level()),
                };
                let model = read(&self.model()).clone();
                match maestro_session::compaction::compact(&preparation, &model, &options).await {
                    Ok(result) => (super::stored_result(result), false),
                    Err(error) => {
                        return Ok(Outcome::Failed {
                            message: error.message,
                        });
                    }
                }
            }
        };
        if signal.is_aborted() {
            return Ok(Outcome::Skipped { aborted: true });
        }
        let entry = self
            .store_compaction(&result, from_extension)
            .map_err(failed)?;
        self.announce_compaction(entry, from_extension)
            .await
            .map_err(failed)?;
        Ok(Outcome::Compacted { result })
    }

    /// The key and headers automatic compaction uses, absent when none are configured.
    async fn configured_auth(&self) -> Option<(String, Option<IndexMap<String, String>>)> {
        let model = self.model();
        let auth = self
            .model_registry
            .get_api_key_and_headers(&model, &*self.config_value_operations)
            .await;
        let maestro_catalog::ResolvedRequestAuth::Ok {
            api_key: Some(api_key),
            headers,
        } = auth
        else {
            return None;
        };
        (!api_key.is_empty()).then_some((api_key, headers))
    }

    /// The context tokens the finished response reports, absent when nothing is known.
    fn context_tokens(
        &self,
        message: &AssistantMessage,
        boundary: Option<&str>,
    ) -> Result<Option<f64>, DiagnosticErrorInfo> {
        if message.stop_reason != StopReason::Error {
            let usage = serde_json::to_value(&message.usage).map_err(failed)?;
            return calculate_context_tokens(&usage).map(Some).map_err(failed);
        }
        let records = crate::agent_session::state::message_records(&read(&self.messages()))?;
        let estimate = estimate_context_tokens(&records).map_err(failed)?;
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
        let branch = manager
            .get_branch(None)
            .map_err(crate::agent_session::state::thrown)?;
        let boundary = get_latest_compaction_entry(&branch)
            .map_err(crate::agent_session::state::thrown)?
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

    /// Whether automatic compaction runs once the context passes its reserve.
    fn compaction_policy(&self) -> maestro_session::compaction::CompactionSettings {
        super::compaction_settings(&self.settings_manager.borrow().get_compaction_settings())
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

/// The thrown form of a lower failure.
fn failed(error: impl std::fmt::Display) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo::error(error.to_string())
}
