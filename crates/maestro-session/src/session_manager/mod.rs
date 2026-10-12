#![doc = include_str!("../../../../docs/sessions/session_manager.md")]
/// Context projection operations.
pub mod context;
/// Session record views.
pub mod types;
pub use context::{build_session_context, get_latest_compaction_entry, parse_session_entries};
pub use types::*;

use chrono::{SecondsFormat, Utc};
use maestro_request::presence::Presence;
use serde_json::{Value, json};
use std::collections::HashMap;
use uuid::Uuid;

/// In-memory history with indexed entries and a selected branch.
pub struct SessionManager {
    /// Authored working directory captured at construction.
    cwd: String,
    /// Session-directory spelling; empty in memory mode.
    session_dir: String,
    /// Current session identity.
    session_id: String,
    /// Header followed by retained entries.
    file_entries: Vec<FileEntry>,
    /// Entry identity to position in the retained sequence.
    by_id: HashMap<String, usize>,
    /// Selected entry identity.
    leaf_id: Option<String>,
}

impl SessionManager {
    /// Create a memory session without session-file reads or writes.
    /// Only an omitted directory reads the process working directory.
    ///
    /// # Errors
    /// Propagates the native working-directory error when the directory is omitted.
    pub fn in_memory(cwd: Option<&str>) -> std::io::Result<Self> {
        let cwd = match cwd {
            Some(cwd) => cwd.to_owned(),
            None => std::env::current_dir()?.to_string_lossy().into_owned(),
        };
        let mut manager = Self {
            cwd,
            session_dir: String::new(),
            session_id: String::new(),
            file_entries: Vec::new(),
            by_id: HashMap::new(),
            leaf_id: None,
        };
        manager.new_session(NewSessionOptions::default());
        Ok(manager)
    }

    /// Replace active history with a new header; a memory session has no file.
    pub fn new_session(&mut self, options: NewSessionOptions) -> Option<&str> {
        self.session_id = options.id.unwrap_or_else(|| Uuid::now_v7().to_string());
        let mut header = json!({
            "type": "session", "version": CURRENT_SESSION_VERSION,
            "id": self.session_id, "timestamp": Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            "cwd": self.cwd,
        });
        match options.parent_session {
            Presence::Missing => {}
            Presence::Null => header["parentSession"] = Value::Null,
            Presence::Present(parent) => header["parentSession"] = parent.into(),
        }
        self.file_entries = vec![header];
        self.by_id.clear();
        self.leaf_id = None;
        None
    }

    /// Append the raw message as a child of the selected entry and select it.
    pub fn append_message(&mut self, message: Value) -> String {
        let id = generate_id(|id| self.by_id.contains_key(id), Uuid::new_v4);
        let mut entry = json!({
            "type": "message", "id": id, "parentId": self.leaf_id,
            "timestamp": Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        });
        entry["message"] = message;
        self.by_id.insert(id.clone(), self.file_entries.len());
        self.file_entries.push(entry);
        self.leaf_id = Some(id.clone());
        id
    }
    /// Borrow the selected entry identity, if any.
    #[must_use]
    pub fn get_leaf_id(&self) -> Option<&str> {
        self.leaf_id.as_deref()
    }
    /// Borrow the selected indexed entry, if any.
    #[must_use]
    pub fn get_leaf_entry(&self) -> Option<&SessionEntry> {
        self.get_leaf_id().and_then(|id| self.get_entry(id))
    }
    /// Borrow an entry by its exact identity.
    #[must_use]
    pub fn get_entry(&self, id: &str) -> Option<&SessionEntry> {
        self.by_id
            .get(id)
            .and_then(|position| self.file_entries.get(*position))
    }
    /// Borrow the retained entries after the generated header.
    #[must_use]
    pub fn get_entries(&self) -> &[SessionEntry] {
        &self.file_entries[1..]
    }

    /// Borrow root-first ancestry from an explicit identity or the selected leaf.
    ///
    /// # Errors
    /// Propagates reached property errors from the shared ancestry walk.
    pub fn get_branch(
        &self,
        from_id: Option<&str>,
    ) -> Result<Vec<&SessionEntry>, serde_json::Error> {
        match from_id
            .or(self.get_leaf_id())
            .and_then(|id| self.get_entry(id))
        {
            Some(leaf) => context::ancestor_path(leaf, |id| self.get_entry(id)),
            None => Ok(Vec::new()),
        }
    }
    /// Project context from the selected branch using the shared projection.
    ///
    /// # Errors
    /// Propagates reached property errors from context projection.
    pub fn build_session_context(&self) -> Result<SessionContext, serde_json::Error> {
        let leaf = self.get_leaf_id().map_or(Presence::Null, Presence::Present);
        context::indexed_context(self.get_entries().last(), leaf, |id| self.get_entry(id))
    }
    /// Select an existing entry without changing retained history.
    ///
    /// # Errors
    /// Returns `Entry {id} not found` when the identity is not indexed.
    pub fn branch(&mut self, id: &str) -> std::io::Result<()> {
        if !self.by_id.contains_key(id) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Entry {id} not found"),
            ));
        }
        self.leaf_id = Some(id.to_owned());
        Ok(())
    }

    /// Whether history is persisted; memory history is not.
    #[must_use]
    pub fn is_persisted(&self) -> bool {
        false
    }
    /// Borrow the captured working-directory spelling.
    #[must_use]
    pub fn get_cwd(&self) -> &str {
        &self.cwd
    }
    /// Memory history has no session directory.
    #[must_use]
    pub fn get_session_dir(&self) -> &str {
        &self.session_dir
    }
    /// Borrow the current session identity.
    #[must_use]
    pub fn get_session_id(&self) -> &str {
        &self.session_id
    }
    /// Memory history has no session file.
    #[must_use]
    pub fn get_session_file(&self) -> Option<&str> {
        None
    }
    /// Borrow the retained current header.
    #[must_use]
    pub fn get_header(&self) -> Option<&FileEntry> {
        self.file_entries.first()
    }
}

/// Generate a free short entry identity, falling back after one hundred collisions.
fn generate_id(
    mut has_id: impl FnMut(&str) -> bool,
    mut next_uuid: impl FnMut() -> Uuid,
) -> String {
    for _ in 0..100 {
        let id = next_uuid().simple().to_string()[..8].to_owned();
        if !has_id(&id) {
            return id;
        }
    }
    next_uuid().to_string()
}

#[cfg(test)]
mod tests;
