//! Manual and automatic compaction over the supplied history.
use super::AgentSession;
use super::state::{agent_messages, thrown};
use crate::utils::lock::write;
use maestro_extensions::{ExtensionExecution, SessionBeforeCompactEvent, SessionCompactEvent};
use maestro_models::{Cancellation, DiagnosticErrorInfo};
use maestro_session::compaction::{
    CompactionPreparation, CompactionResult, CompactionSettings, prepare_compaction,
};
use maestro_session::{
    CompactionEntry, Presence, SessionEntry,
    session_manager::{CompactionOptions, ReadonlySessionManager},
};
use maestro_settings::ResolvedCompactionSettings;
use serde_json::Value;

mod automatic;
mod manual;

/// What a before-compaction handler decided.
enum BeforeCompaction {
    /// A handler supplied the summary.
    Supplied {
        /// The authored compaction.
        result: CompactionResult,
    },
    /// No handler supplied content.
    Continue,
    /// A handler refused the compaction.
    Cancelled,
}

/// The branch a compaction is prepared over.
struct PreparedBranch {
    /// Ordered branch entries.
    entries: Vec<SessionEntry>,
    /// The prepared compaction, absent when the branch cannot be cut.
    preparation: Option<CompactionPreparation>,
}

impl<E: ExtensionExecution> AgentSession<E> {
    /// The branch entries, the policy and the prepared compaction.
    fn prepared_branch(&self) -> Result<PreparedBranch, DiagnosticErrorInfo> {
        let manager = self.session_manager.borrow();
        let entries: Vec<SessionEntry> = manager
            .get_branch(None)
            .map_err(thrown)?
            .into_iter()
            .cloned()
            .collect();
        let settings =
            compaction_settings(&self.settings_manager.borrow().get_compaction_settings());
        let preparation = prepare_compaction(&entries, &settings).map_err(thrown)?;
        drop(manager);
        Ok(PreparedBranch {
            entries,
            preparation,
        })
    }

    /// Offers the pending compaction to handlers before any summary is requested.
    async fn before_compaction(
        &self,
        preparation: &CompactionPreparation,
        branch_entries: Vec<SessionEntry>,
        custom_instructions: Option<String>,
        signal: &Cancellation,
    ) -> Result<BeforeCompaction, DiagnosticErrorInfo> {
        if !self.extension_runner.has_handlers("session_before_compact") {
            return Ok(BeforeCompaction::Continue);
        }
        let mut event = SessionBeforeCompactEvent {
            data: maestro_extensions::SessionBeforeCompactEventData {
                preparation: preparation.clone(),
                branch_entries,
                custom_instructions: presence(custom_instructions),
            },
            signal: signal.clone(),
        };
        let Some(result) = self
            .extension_runner
            .emit(&mut event)
            .await
            .map_err(thrown)?
        else {
            return Ok(BeforeCompaction::Continue);
        };
        if matches!(result.cancel, Presence::Present(true)) {
            return Ok(BeforeCompaction::Cancelled);
        }
        Ok(match result.compaction.present() {
            Some(result) => BeforeCompaction::Supplied {
                result: result.clone(),
            },
            None => BeforeCompaction::Continue,
        })
    }

    /// Stores the compaction, rebuilds the context and returns the stored entry.
    fn store_compaction(
        &self,
        result: &CompactionResult,
        from_extension: bool,
    ) -> Result<Option<CompactionEntry>, DiagnosticErrorInfo> {
        let options = CompactionOptions {
            details: result.details.clone(),
            from_hook: Presence::Present(from_extension),
        };
        let mut manager = self.session_manager.borrow_mut();
        manager
            .append_compaction(
                &result.summary,
                &result.first_kept_entry_id,
                result.tokens_before,
                options,
            )
            .map_err(thrown)?;
        let entry = manager.get_entries().into_iter().find_map(|entry| {
            let is_match = entry.get("type").and_then(Value::as_str) == Some("compaction")
                && entry.get("summary").and_then(Value::as_str) == Some(&result.summary);
            is_match
                .then(|| serde_json::from_value::<CompactionEntry>(entry.clone()).ok())
                .flatten()
        });
        let context = manager.build_session_context().map_err(thrown)?;
        drop(manager);
        let messages = agent_messages(context.messages)?;
        *write(&self.messages()) = messages;
        Ok(entry)
    }

    /// Announces a stored compaction entry to handlers.
    async fn announce_compaction(
        &self,
        entry: Option<CompactionEntry>,
        from_extension: bool,
    ) -> Result<(), DiagnosticErrorInfo> {
        let Some(entry) = entry else {
            return Ok(());
        };
        let mut event = SessionCompactEvent {
            compaction_entry: entry,
            from_extension,
        };
        self.extension_runner
            .emit(&mut event)
            .await
            .map_err(thrown)?;
        Ok(())
    }
}

/// The compaction policy the resolved settings describe.
fn compaction_settings(settings: &ResolvedCompactionSettings) -> CompactionSettings {
    CompactionSettings {
        enabled: settings.enabled,
        reserve_tokens: settings.reserve_tokens,
        keep_recent_tokens: settings.keep_recent_tokens,
    }
}

/// A supplied optional text as an absent-or-present property.
fn presence(value: Option<String>) -> Presence<String> {
    value.map_or(Presence::Missing, Presence::Present)
}

/// The compaction a generated summary stores.
pub(super) fn stored_result(
    result: CompactionResult<maestro_session::CompactionDetails>,
) -> CompactionResult {
    CompactionResult {
        summary: result.summary,
        first_kept_entry_id: result.first_kept_entry_id,
        tokens_before: result.tokens_before,
        details: generated_details(&result.details),
    }
}

/// The details a generated compaction carried, as the stored record admits them.
fn generated_details(details: &Presence<maestro_session::CompactionDetails>) -> Presence<Value> {
    match details {
        Presence::Missing => Presence::Missing,
        Presence::Null => Presence::Null,
        Presence::Present(details) => {
            serde_json::to_value(details).map_or(Presence::Null, Presence::Present)
        }
    }
}
