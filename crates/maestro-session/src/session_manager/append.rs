//! Entry append operations sharing one memory-before-publication path.
use super::{
    BranchSummaryEntry, BranchSummaryOptions, CompactionEntry, CompactionOptions, CustomEntry,
    CustomMessageEntry, ModelChangeEntry, SessionEntryBase, SessionInfoEntry, SessionManager,
    SessionMessageEntry, ThinkingLevelChangeEntry, context::ecma_whitespace, generate_id,
};
use chrono::{SecondsFormat, Utc};
use maestro_models::UserContent;
use maestro_request::presence::Presence;
use serde::Serialize;
use serde_json::Value;
use std::io;
use uuid::Uuid;

impl SessionManager {
    /// Accept a record below the selected entry, select it, then publish it.
    ///
    /// Publication errors leave the accepted memory and any written bytes in place.
    pub(super) fn append_record<T: Serialize>(
        &mut self,
        record: impl FnOnce(SessionEntryBase) -> T,
    ) -> io::Result<String> {
        let id = generate_id(|id| self.by_id.contains_key(id), Uuid::new_v4);
        let base = SessionEntryBase {
            id: id.clone(),
            parent_id: self.leaf_id.clone(),
            timestamp: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        };
        let entry = serde_json::to_value(record(base))?;
        self.by_id.insert(id.clone(), self.file_entries.len());
        self.file_entries.push(entry);
        self.leaf_id = Some(id.clone());
        #[cfg(not(target_arch = "wasm32"))]
        self.persist_entry()?;
        Ok(id)
    }

    /// Append the raw message as a child of the selected entry and select it.
    ///
    /// # Errors
    /// Publication errors leave the accepted memory and any written bytes in place.
    pub fn append_message(&mut self, message: Value) -> io::Result<String> {
        self.append_record(|base| SessionMessageEntry { base, message })
    }

    /// Record the selected thinking level.
    ///
    /// # Errors
    /// Publication errors leave the accepted memory and any written bytes in place.
    pub fn append_thinking_level_change(&mut self, thinking_level: &str) -> io::Result<String> {
        self.append_record(|base| ThinkingLevelChangeEntry {
            base,
            thinking_level: thinking_level.to_owned(),
        })
    }

    /// Record the selected provider and model pair.
    ///
    /// # Errors
    /// Publication errors leave the accepted memory and any written bytes in place.
    pub fn append_model_change(&mut self, provider: &str, model_id: &str) -> io::Result<String> {
        self.append_record(|base| ModelChangeEntry {
            base,
            provider: provider.to_owned(),
            model_id: model_id.to_owned(),
        })
    }

    /// Record supplied compaction data; the cut identity is never checked.
    ///
    /// # Errors
    /// Publication errors leave the accepted memory and any written bytes in place.
    pub fn append_compaction(
        &mut self,
        summary: &str,
        first_kept_entry_id: &str,
        tokens_before: f64,
        options: CompactionOptions,
    ) -> io::Result<String> {
        self.append_record(|base| CompactionEntry {
            base,
            summary: summary.to_owned(),
            first_kept_entry_id: first_kept_entry_id.to_owned(),
            tokens_before,
            details: options.details,
            from_hook: options.from_hook,
            usage: options.usage,
        })
    }

    /// Retain extension state that never enters model context.
    ///
    /// # Errors
    /// Publication errors leave the accepted memory and any written bytes in place.
    pub fn append_custom_entry(
        &mut self,
        custom_type: &str,
        data: Presence<Value>,
    ) -> io::Result<String> {
        self.append_record(|base| CustomEntry {
            custom_type: custom_type.to_owned(),
            data,
            base,
        })
    }

    /// Record a session name with outer ECMAScript whitespace removed; an empty name clears it.
    ///
    /// # Errors
    /// Publication errors leave the accepted memory and any written bytes in place.
    pub fn append_session_info(&mut self, name: &str) -> io::Result<String> {
        let name = Presence::Present(name.trim_matches(ecma_whitespace).to_owned());
        self.append_record(|base| SessionInfoEntry { base, name })
    }

    /// Retain injected content that joins model context whether or not it is displayed.
    ///
    /// # Errors
    /// Publication errors leave the accepted memory and any written bytes in place.
    pub fn append_custom_message_entry(
        &mut self,
        custom_type: &str,
        content: UserContent,
        display: bool,
        details: Presence<Value>,
    ) -> io::Result<String> {
        self.append_record(|base| CustomMessageEntry {
            custom_type: custom_type.to_owned(),
            content,
            display,
            details,
            base,
        })
    }

    /// Select an existing entry, or the position before all entries, and append its supplied summary.
    ///
    /// # Errors
    /// Returns `Entry {id} not found` before any change when the branch point is not indexed;
    /// publication errors leave the accepted memory and any written bytes in place.
    pub fn branch_with_summary(
        &mut self,
        branch_from_id: Option<&str>,
        summary: &str,
        options: BranchSummaryOptions,
    ) -> io::Result<String> {
        if let Some(id) = branch_from_id {
            self.require_entry(id)?;
        }
        self.leaf_id = branch_from_id.map(str::to_owned);
        self.append_record(|base| BranchSummaryEntry {
            base,
            from_id: branch_from_id.unwrap_or("root").to_owned(),
            summary: summary.to_owned(),
            details: options.details,
            from_hook: options.from_hook,
            usage: options.usage,
        })
    }
}
