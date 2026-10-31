#![doc = include_str!("../../../../docs/sessions/session_manager.md")]
mod append;
mod conversions;
#[cfg(not(target_arch = "wasm32"))]
mod discovery;
mod extraction;
#[cfg(not(target_arch = "wasm32"))]
mod files;
#[cfg(not(target_arch = "wasm32"))]
mod persistence;
#[cfg(not(target_arch = "wasm32"))]
mod storage;
pub use conversions::migrate_session_entries;
#[cfg(not(target_arch = "wasm32"))]
pub use discovery::SessionListProgress;
#[cfg(not(target_arch = "wasm32"))]
pub use files::{
    SessionDir, find_most_recent_session, get_default_session_dir, load_entries_from_file,
    sessions_dir,
};
#[cfg(not(target_arch = "wasm32"))]
pub use storage::rewrite_session_file;
/// Context projection operations.
pub mod context;
mod tree;
/// Session record views.
pub mod types;
pub use context::{build_session_context, get_latest_compaction_entry, parse_session_entries};
pub use types::*;

use chrono::{SecondsFormat, Utc};
use maestro_request::presence::Presence;
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use uuid::Uuid;

/// Retained history with indexed entries and a selected branch.
pub struct SessionManager {
    /// Authored working directory captured at construction.
    cwd: String,
    /// Session-directory spelling; empty in memory mode.
    session_dir: String,
    /// Whether native publication is enabled.
    persist: bool,
    /// Whether the active history has already been published.
    flushed: bool,
    /// Selected native filename, including before publication.
    session_file: Option<String>,
    /// Current session identity.
    session_id: String,
    /// Header followed by retained entries.
    file_entries: Vec<FileEntry>,
    /// Entry identity to position in the retained sequence.
    by_id: HashMap<String, usize>,
    /// Selected entry identity.
    leaf_id: Option<String>,
    /// Resolved raw label per target identity, in first-set order.
    labels: Map<String, Value>,
    /// Resolved raw label timestamp per target identity.
    label_timestamps: Map<String, Value>,
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
        let mut manager = Self::empty(cwd, String::new(), false);
        manager.new_session(NewSessionOptions::default());
        Ok(manager)
    }

    /// Initialize empty retained state, creating a native directory when configured.
    #[cfg(not(target_arch = "wasm32"))]
    fn base(cwd: String, session_dir: String, persist: bool) -> std::io::Result<Self> {
        if persist && !session_dir.is_empty() && !std::path::Path::new(&session_dir).exists() {
            std::fs::create_dir_all(&session_dir)?;
        }
        Ok(Self::empty(cwd, session_dir, persist))
    }

    /// Initialize retained state without filesystem effects.
    fn empty(cwd: String, session_dir: String, persist: bool) -> Self {
        Self {
            cwd,
            session_dir,
            persist,
            flushed: false,
            session_file: None,
            session_id: String::new(),
            file_entries: Vec::new(),
            by_id: HashMap::new(),
            leaf_id: None,
            labels: Map::new(),
            label_timestamps: Map::new(),
        }
    }

    /// Replace history without writing; native mode selects a fresh filename.
    pub fn new_session(&mut self, options: NewSessionOptions) -> Option<&str> {
        self.session_id = options.id.unwrap_or_else(|| Uuid::now_v7().to_string());
        let (header, file) = session_header(&self.session_id, &self.cwd, options.parent_session);
        if self.persist {
            self.session_file = Some(maestro_path::join(&[&self.session_dir, &file]));
        }
        self.flushed = false;
        self.file_entries = vec![header];
        self.by_id.clear();
        self.labels.clear();
        self.label_timestamps.clear();
        self.leaf_id = None;
        self.session_file.as_deref()
    }

    /// Project context from the selected branch using the shared projection.
    ///
    /// # Errors
    /// Propagates reached property errors from context projection.
    pub fn build_session_context(&self) -> Result<SessionContext, serde_json::Error> {
        let leaf = self.get_leaf_id().map_or(Presence::Null, Presence::Present);
        context::indexed_context(self.get_entries().last().copied(), leaf, |id| {
            self.get_entry(id)
        })
    }
    /// Select an existing entry without changing retained history.
    ///
    /// # Errors
    /// Returns `Entry {id} not found` when the identity is not indexed.
    pub fn branch(&mut self, id: &str) -> std::io::Result<()> {
        self.require_entry(id)?;
        self.leaf_id = Some(id.to_owned());
        Ok(())
    }

    /// Fail with `Entry {id} not found` unless the identity is indexed.
    fn require_entry(&self, id: &str) -> std::io::Result<()> {
        let missing = || {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Entry {id} not found"),
            )
        };
        self.by_id
            .contains_key(id)
            .then_some(())
            .ok_or_else(missing)
    }

    /// Whether native publication is enabled.
    #[must_use]
    pub fn is_persisted(&self) -> bool {
        self.persist
    }
}

/// Build a current-version header and its lexical native filename.
fn session_header(id: &str, cwd: &str, parent: Presence<String>) -> (FileEntry, String) {
    let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let file = format!("{}_{id}.jsonl", timestamp.replace([':', '.'], "-"));
    let mut header = json!({
        "type": "session", "version": CURRENT_SESSION_VERSION,
        "id": id, "timestamp": timestamp, "cwd": cwd,
    });
    match parent {
        Presence::Missing => {}
        Presence::Null => header["parentSession"] = Value::Null,
        Presence::Present(parent) => header["parentSession"] = parent.into(),
    }
    (header, file)
}

/// Index retained records, select the last identity and resolve labels in order.
///
/// # Errors
/// Returns invalid data for a retained record whose identity is not a string.
fn build_index(
    entries: &[FileEntry],
    by_id: &mut HashMap<String, usize>,
    leaf_id: &mut Option<String>,
    labels: &mut Map<String, Value>,
    label_timestamps: &mut Map<String, Value>,
) -> std::io::Result<()> {
    for (position, entry) in entries.iter().enumerate() {
        if entry["type"] == "session" {
            continue;
        }
        let id = entry["id"]
            .as_str()
            .ok_or_else(|| conversions::invalid("nonstring entry id"))?;
        by_id.insert(id.to_owned(), position);
        *leaf_id = Some(id.to_owned());
        if entry["type"] == "label" {
            tree::apply_label(labels, label_timestamps, entry);
        }
    }
    Ok(())
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
