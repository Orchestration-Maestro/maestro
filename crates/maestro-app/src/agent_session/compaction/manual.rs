//! The compaction a caller asks for.
use super::super::{AgentSession, AgentSessionEvent, CompactionReason};
use super::BeforeCompaction;
use maestro_extensions::ExtensionExecution;
use maestro_models::{Cancellation, DiagnosticErrorInfo};
use maestro_session::CompactionResult;
use maestro_session::compaction::SummaryOptions;

impl<E: ExtensionExecution> AgentSession<E> {
    /// Compacts the session context now, after stopping the running turn.
    ///
    /// # Errors
    /// Returns `Already compacted` or `Nothing to compact (session too small)` when the branch
    /// cannot be cut, `Compaction cancelled` when a handler or caller stopped it, and the
    /// authentication or summarizer failure otherwise.
    pub async fn compact(
        &self,
        custom_instructions: Option<&str>,
    ) -> Result<CompactionResult, DiagnosticErrorInfo> {
        self.abort().await;
        let signal = Cancellation::new();
        self.open_compaction(CompactionReason::Manual, &signal);
        let outcome = self.compact_context(&signal, custom_instructions).await;
        self.close_compaction(CompactionReason::Manual);
        match outcome {
            Ok(result) => {
                self.emit(&AgentSessionEvent::CompactionEnd {
                    reason: CompactionReason::Manual,
                    result: Some(result.clone()),
                    aborted: false,
                    will_retry: false,
                    error_message: None,
                });
                Ok(result)
            }
            Err(error) => {
                let aborted = error.message == "Compaction cancelled" || signal.is_aborted();
                self.emit(&AgentSessionEvent::CompactionEnd {
                    reason: CompactionReason::Manual,
                    result: None,
                    aborted,
                    will_retry: false,
                    error_message: (!aborted)
                        .then(|| format!("Compaction failed: {}", error.message)),
                });
                Err(error)
            }
        }
    }

    /// Cancels an in-progress manual or automatic compaction.
    pub fn abort_compaction(&self) {
        let (manual, automatic) = {
            let state = self.state.borrow();
            (
                state.compaction_signal.clone(),
                state.auto_compaction_signal.clone(),
            )
        };
        for signal in manual.into_iter().chain(automatic) {
            signal.abort();
        }
    }

    /// Cancels an in-progress branch summarization.
    pub fn abort_branch_summary(&self) {
        let signal = self.state.borrow().branch_summary_signal.clone();
        if let Some(signal) = signal {
            signal.abort();
        }
    }

    /// Whether a compaction or branch summarization is running.
    #[must_use]
    pub fn is_compacting(&self) -> bool {
        let state = self.state.borrow();
        state.compaction_signal.is_some()
            || state.auto_compaction_signal.is_some()
            || state.branch_summary_signal.is_some()
    }

    /// Whether automatic compaction runs once the context passes the reserve.
    #[must_use]
    pub fn auto_compaction_enabled(&self) -> bool {
        self.settings_manager.borrow().get_compaction_enabled()
    }

    /// Enables or disables automatic compaction.
    pub fn set_auto_compaction_enabled(&self, enabled: bool) {
        self.settings_manager
            .borrow_mut()
            .set_compaction_enabled(enabled);
    }

    /// Compacts the context once the caller's turn is stopped.
    async fn compact_context(
        &self,
        signal: &Cancellation,
        custom_instructions: Option<&str>,
    ) -> Result<CompactionResult, DiagnosticErrorInfo> {
        let model = self.model();
        let (api_key, headers) = self.get_required_request_auth(&model).await?;
        let prepared = self.prepared_branch()?;
        let Some(preparation) = prepared.preparation else {
            let last_is_compaction = prepared
                .entries
                .last()
                .and_then(|entry| entry.get("type"))
                .and_then(serde_json::Value::as_str)
                == Some("compaction");
            return Err(DiagnosticErrorInfo::error(if last_is_compaction {
                "Already compacted"
            } else {
                "Nothing to compact (session too small)"
            }));
        };
        let supplied = self
            .before_compaction(
                &preparation,
                prepared.entries.clone(),
                custom_instructions.map(str::to_owned),
                signal,
            )
            .await?;
        let (result, from_extension) = match supplied {
            BeforeCompaction::Cancelled => {
                return Err(DiagnosticErrorInfo::error("Compaction cancelled"));
            }
            BeforeCompaction::Supplied { result } => (result, true),
            BeforeCompaction::Continue => {
                let options = SummaryOptions {
                    api_key,
                    headers,
                    signal: Some(signal.clone()),
                    custom_instructions: custom_instructions.map(str::to_owned),
                    thinking_level: Some(self.thinking_level()),
                };
                let model = crate::utils::lock::read(&model).clone();
                let result =
                    maestro_session::compaction::compact(&preparation, &model, &options).await?;
                (
                    CompactionResult {
                        summary: result.summary,
                        first_kept_entry_id: result.first_kept_entry_id,
                        tokens_before: result.tokens_before,
                        details: super::generated_details(&result.details),
                    },
                    false,
                )
            }
        };
        if signal.is_aborted() {
            return Err(DiagnosticErrorInfo::error("Compaction cancelled"));
        }
        let entry = self.store_compaction(&result, from_extension)?;
        self.announce_compaction(entry, from_extension).await?;
        Ok(result)
    }

    /// Records the running compaction before its start event is announced.
    fn open_compaction(&self, reason: CompactionReason, signal: &Cancellation) {
        {
            let mut state = self.state.borrow_mut();
            match reason {
                CompactionReason::Manual => state.compaction_signal = Some(signal.clone()),
                _ => state.auto_compaction_signal = Some(signal.clone()),
            }
        }
        self.emit(&AgentSessionEvent::CompactionStart { reason });
    }

    /// Clears the running compaction after its end event is announced.
    fn close_compaction(&self, reason: CompactionReason) {
        {
            let mut state = self.state.borrow_mut();
            match reason {
                CompactionReason::Manual => state.compaction_signal = None,
                _ => state.auto_compaction_signal = None,
            }
        }
    }
}
