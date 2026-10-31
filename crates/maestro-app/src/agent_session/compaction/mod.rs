//! Manual and automatic compaction over the supplied history.
use super::navigation::presence;
use super::state::{agent_messages, thrown};
use super::{AgentSession, AgentSessionEvent, CompactionReason};
use crate::utils::lock::{read, write};
use indexmap::IndexMap;
use maestro_extensions::{ExtensionExecution, SessionBeforeCompactEvent, SessionCompactEvent};
use maestro_models::{Cancellation, DiagnosticErrorInfo};
use maestro_session::compaction::{
    CompactionPreparation, CompactionResult, CompactionSettings, SummaryOptions, prepare_compaction,
};
use maestro_session::{
    CompactionEntry, Presence, SessionEntry,
    session_manager::{CompactionOptions, ReadonlySessionManager},
};
use serde::Deserialize;
use serde_json::Value;

mod automatic;
mod manual;

/// What a before-compaction handler decided.
enum BeforeCompaction {
    /// A handler supplied the summary.
    Supplied {
        /// The authored compaction.
        result: Box<CompactionResult>,
    },
    /// No handler supplied content; generation reads the preparation handlers saw.
    Continue {
        /// The preparation as the handlers left it.
        preparation: Box<CompactionPreparation>,
    },
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
    /// The compaction policy the saved preferences describe now.
    fn compaction_policy(&self) -> CompactionSettings {
        let settings = self.settings_manager.borrow().get_compaction_settings();
        CompactionSettings {
            enabled: settings.enabled,
            reserve_tokens: settings.reserve_tokens,
            keep_recent_tokens: settings.keep_recent_tokens,
        }
    }

    /// The branch entries and the compaction `settings` prepare over them.
    fn prepared_branch(
        &self,
        settings: &CompactionSettings,
    ) -> Result<PreparedBranch, DiagnosticErrorInfo> {
        let manager = self.session_manager.borrow();
        let entries: Vec<SessionEntry> = manager
            .get_branch(None)
            .map_err(thrown)?
            .into_iter()
            .cloned()
            .collect();
        drop(manager);
        let preparation = prepare_compaction(&entries, settings).map_err(thrown)?;
        Ok(PreparedBranch {
            entries,
            preparation,
        })
    }

    /// Offers the pending compaction to handlers before any summary is requested.
    async fn before_compaction(
        &self,
        preparation: CompactionPreparation,
        branch_entries: Vec<SessionEntry>,
        custom_instructions: Option<String>,
        signal: &Cancellation,
    ) -> Result<BeforeCompaction, DiagnosticErrorInfo> {
        if !self
            .extension_runner()
            .has_handlers("session_before_compact")
        {
            return Ok(BeforeCompaction::Continue {
                preparation: Box::new(preparation),
            });
        }
        let mut event = SessionBeforeCompactEvent {
            data: maestro_extensions::SessionBeforeCompactEventData {
                preparation,
                branch_entries,
                custom_instructions: presence(custom_instructions),
            },
            signal: signal.clone(),
        };
        let result = self
            .extension_runner()
            .emit(&mut event)
            .await
            .map_err(thrown)?;
        let preparation = event.data.preparation;
        let Some(result) = result else {
            return Ok(BeforeCompaction::Continue {
                preparation: Box::new(preparation),
            });
        };
        if matches!(result.cancel, Presence::Present(true)) {
            return Ok(BeforeCompaction::Cancelled);
        }
        Ok(match result.compaction {
            Presence::Present(result) => BeforeCompaction::Supplied {
                result: Box::new(result),
            },
            Presence::Missing | Presence::Null => BeforeCompaction::Continue {
                preparation: Box::new(preparation),
            },
        })
    }

    /// Generates the summary of `preparation` with the selected model and thinking level.
    async fn summarize(
        &self,
        preparation: &CompactionPreparation,
        (api_key, headers): (String, Option<IndexMap<String, String>>),
        custom_instructions: Option<String>,
        signal: &Cancellation,
    ) -> Result<CompactionResult, DiagnosticErrorInfo> {
        let options = SummaryOptions {
            api_key,
            headers,
            signal: Some(signal.clone()),
            custom_instructions,
            thinking_level: Some(self.thinking_level()),
        };
        let model = read(&self.model()).clone();
        maestro_session::compaction::compact(preparation, &model, &options)
            .await
            .map(stored_result)
    }

    /// Stores the compaction, rebuilds the context and returns the entry just stored.
    fn store_compaction(
        &self,
        result: &CompactionResult,
        from_extension: bool,
    ) -> Result<Option<CompactionEntry>, DiagnosticErrorInfo> {
        let options = CompactionOptions {
            details: result.details.clone(),
            from_hook: Presence::Present(from_extension),
            usage: Presence::Missing,
        };
        let mut manager = self.session_manager.borrow_mut();
        let id = manager
            .append_compaction(
                &result.summary,
                &result.first_kept_entry_id,
                result.tokens_before,
                options,
            )
            .map_err(thrown)?;
        let entry = manager
            .get_entry(&id)
            .and_then(|entry| CompactionEntry::deserialize(entry).ok());
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
        self.extension_runner()
            .emit(&mut event)
            .await
            .map_err(thrown)?;
        Ok(())
    }

    /// Announces a compaction's end, then clears its running signal.
    fn close_compaction(&self, end: &AgentSessionEvent) {
        self.emit(end);
        let mut state = self.state.borrow_mut();
        if matches!(
            end,
            AgentSessionEvent::CompactionEnd {
                reason: CompactionReason::Manual,
                ..
            }
        ) {
            state.compaction_signal = None;
        } else {
            state.auto_compaction_signal = None;
        }
    }
}

/// The compaction a generated summary stores.
fn stored_result(result: CompactionResult<maestro_session::CompactionDetails>) -> CompactionResult {
    CompactionResult {
        summary: result.summary,
        first_kept_entry_id: result.first_kept_entry_id,
        tokens_before: result.tokens_before,
        details: generated_details(&result.details),
        usage: Presence::Missing,
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
