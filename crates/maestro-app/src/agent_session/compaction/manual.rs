//! The compaction a caller asks for.
use super::super::state::thrown;
use super::super::{AgentSession, AgentSessionEvent, CompactionReason};
use super::BeforeCompaction;
use maestro_extensions::ExtensionExecution;
use maestro_models::{Cancellation, DiagnosticErrorInfo};
use maestro_session::CompactionResult;
use std::rc::Rc;

impl<E: ExtensionExecution> AgentSession<E> {
    /// Disconnects event forwarding and stops the running turn when called, then compacts
    /// after the agent settles. The compaction belongs to the session: dropping even the
    /// never-polled result future leaves it running, and `abort_compaction` stops it.
    ///
    /// # Errors
    /// Returns `Already compacted` or `Nothing to compact (session too small)` when the branch
    /// cannot be cut, `Compaction cancelled` when a handler or caller stopped it, the
    /// authentication or summarizer failure otherwise, or the failure when the compaction's
    /// task ended before it did.
    pub fn compact(
        self: &Rc<Self>,
        custom_instructions: Option<&str>,
    ) -> impl std::future::Future<Output = Result<CompactionResult, DiagnosticErrorInfo>> + use<E>
    {
        let session = Rc::clone(self);
        let custom_instructions = custom_instructions.map(str::to_owned);
        let result = self.start_operation(async move {
            session
                .manual_compaction(custom_instructions.as_deref())
                .await
        });
        async move { result.await.map_err(thrown)? }
    }

    /// The manual compaction the session runs, announced from start to end, with agent events
    /// kept from the session until it ends.
    async fn manual_compaction(
        &self,
        custom_instructions: Option<&str>,
    ) -> Result<CompactionResult, DiagnosticErrorInfo> {
        self.disconnect_from_agent();
        self.abort().await;
        let signal = Cancellation::new();
        self.state.borrow_mut().compaction_signal = Some(signal.clone());
        self.emit(&AgentSessionEvent::CompactionStart {
            reason: CompactionReason::Manual,
        });
        let outcome = self.compact_context(&signal, custom_instructions).await;
        let end = match &outcome {
            Ok(result) => AgentSessionEvent::CompactionEnd {
                reason: CompactionReason::Manual,
                result: Some(result.clone()),
                aborted: false,
                will_retry: false,
                error_message: None,
            },
            Err(error) => {
                let aborted = error.message == "Compaction cancelled" || signal.is_aborted();
                AgentSessionEvent::CompactionEnd {
                    reason: CompactionReason::Manual,
                    result: None,
                    aborted,
                    will_retry: false,
                    error_message: (!aborted)
                        .then(|| format!("Compaction failed: {}", error.message)),
                }
            }
        };
        self.close_compaction(&end);
        self.reconnect_to_agent();
        outcome
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
    ///
    /// # Errors
    /// Returns the publisher's refusal without changing preferences.
    pub fn set_auto_compaction_enabled(&self, enabled: bool) -> Result<(), DiagnosticErrorInfo> {
        self.settings_manager
            .borrow_mut()
            .set_compaction_enabled(enabled);
        Ok(())
    }

    /// Compacts the context once the caller's turn is stopped.
    async fn compact_context(
        &self,
        signal: &Cancellation,
        custom_instructions: Option<&str>,
    ) -> Result<CompactionResult, DiagnosticErrorInfo> {
        let auth = self.get_required_request_auth(&self.model()).await?;
        let prepared = self.prepared_branch(&self.compaction_policy())?;
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
        let custom_instructions = custom_instructions.map(str::to_owned);
        let supplied = self
            .before_compaction(
                preparation,
                prepared.entries,
                custom_instructions.clone(),
                signal,
            )
            .await?;
        let (result, from_extension) = match supplied {
            BeforeCompaction::Cancelled => {
                return Err(DiagnosticErrorInfo::error("Compaction cancelled"));
            }
            BeforeCompaction::Supplied { result } => (result, true),
            BeforeCompaction::Continue { preparation } => (
                self.summarize(&preparation, auth, custom_instructions, signal)
                    .await?,
                false,
            ),
        };
        if signal.is_aborted() {
            return Err(DiagnosticErrorInfo::error("Compaction cancelled"));
        }
        let entry = self.store_compaction(&result, from_extension)?;
        self.announce_compaction(entry, from_extension).await?;
        Ok(result)
    }
}
