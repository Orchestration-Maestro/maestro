//! Raw entries and explicit session record views.
#![doc = include_str!("../../../../docs/sessions/session_format.md")]
pub use entry::{
    BranchSummaryEntry, CompactionEntry, CustomEntry, CustomMessageEntry, ForkPosition, LabelEntry,
    ModelChangeEntry, NavigateTreeOptions, SessionEntryBase, SessionHeader, SessionInfoEntry,
    ThinkingLevelChangeEntry,
};
use maestro_entry::session_manager as entry;
use maestro_request::presence::Presence;
use serde_json::Value;

/// Unvalidated session entry.
pub type SessionEntry = Value;
/// Message entry holding the stored message as a raw value.
pub type SessionMessageEntry = entry::SessionMessageEntry<Value>;
/// Surviving provider and model observations, each kept missing, null or raw.
pub type SessionContextModel = entry::SessionContextModel<Presence<Value>>;
/// Owned context values; typed messages are explicit later reads.
pub type SessionContext = entry::SessionContext<Value, Presence<Value>>;
/// Identity and parent metadata for a new session.
pub type NewSessionOptions = entry::NewSessionOptions<Presence<String>>;
/// Unvalidated parsed file value.
pub type FileEntry = Value;

/// Current session format version.
pub const CURRENT_SESSION_VERSION: u32 = 3;

/// Supplied optional operands of a compaction record; all are omitted by default.
#[derive(Debug, Default)]
pub struct CompactionOptions {
    /// Extension-specific details.
    pub details: Presence<Value>,
    /// Whether an extension generated the summary.
    pub from_hook: Presence<bool>,
    /// Usage of the model calls that generated the summary.
    pub usage: Presence<maestro_request::types::Usage>,
}

/// Supplied optional operands of a branch-summary record; all are omitted by default.
#[derive(Debug, Default)]
pub struct BranchSummaryOptions {
    /// Extension-specific details.
    pub details: Presence<Value>,
    /// Whether an extension generated the summary.
    pub from_hook: Presence<bool>,
    /// Usage of the model calls that generated the summary.
    pub usage: Presence<maestro_request::types::Usage>,
}

/// One position in the history tree, borrowing retained records and resolved labels.
///
/// Child containers are owned by each returned node; dropping a node walks them
/// with a worklist, so deep histories never grow the call stack.
pub struct SessionTreeNode<'a> {
    /// Retained record observed at this position.
    pub entry: &'a SessionEntry,
    /// Child positions in chronological order.
    pub children: Vec<SessionTreeNode<'a>>,
    /// Resolved label, if any.
    pub label: Option<&'a Value>,
    /// Timestamp of the resolved label's record, if a label resolves; an explicit null stays null.
    pub label_timestamp: Option<&'a Value>,
}

impl Drop for SessionTreeNode<'_> {
    fn drop(&mut self) {
        let mut pending = std::mem::take(&mut self.children);
        while let Some(mut node) = pending.pop() {
            pending.append(&mut node.children);
        }
    }
}

/// Listing metadata of one stored session; dates are epoch milliseconds and `NaN` marks an invalid created date.
#[derive(Debug)]
pub struct SessionInfo {
    /// Session file path.
    pub path: String,
    /// Header identity, passed through unvalidated.
    pub id: Presence<Value>,
    /// Header working directory; empty when the header holds no string.
    pub cwd: String,
    /// Latest trimmed session name; `None` when absent or cleared.
    pub name: Option<String>,
    /// Header parent session, passed through unvalidated.
    pub parent_session_path: Presence<Value>,
    /// Header creation time.
    pub created: f64,
    /// Greatest positive user or assistant activity, else a valid header time string, else file mtime.
    pub modified: f64,
    /// Number of message records.
    pub message_count: usize,
    /// First nonempty user text, or `(no messages)`.
    pub first_message: String,
    /// User and assistant text joined by single spaces.
    pub all_messages_text: String,
}

/// Read-only view of a session history.
pub trait ReadonlySessionManager {
    /// Borrow the captured working-directory spelling.
    fn get_cwd(&self) -> &str;
    /// Borrow the configured session-directory spelling.
    fn get_session_dir(&self) -> &str;
    /// Borrow the current session identity.
    fn get_session_id(&self) -> &str;
    /// Borrow the selected filename, including before publication.
    fn get_session_file(&self) -> Option<&str>;
    /// Borrow the selected entry identity, if any.
    fn get_leaf_id(&self) -> Option<&str>;
    /// Borrow the selected indexed entry, if any.
    fn get_leaf_entry(&self) -> Option<&SessionEntry>;
    /// Borrow an entry by its exact identity.
    fn get_entry(&self, id: &str) -> Option<&SessionEntry>;
    /// Borrow the resolved raw label of an entry identity.
    fn get_label(&self, id: &str) -> Option<&Value>;
    /// Borrow root-first ancestry from an explicit identity or the selected leaf.
    ///
    /// # Errors
    /// Propagates reached property errors from the shared ancestry walk.
    fn get_branch(&self, from_id: Option<&str>) -> Result<Vec<&SessionEntry>, serde_json::Error>;
    /// Borrow the retained current header.
    fn get_header(&self) -> Option<&FileEntry>;
    /// Borrow nonheader records in retained order.
    fn get_entries(&self) -> Vec<&SessionEntry>;
    /// Build the history tree over the retained records.
    fn get_tree(&self) -> Vec<SessionTreeNode<'_>>;
    /// Borrow the name of the newest session-info record, if it holds one.
    ///
    /// # Errors
    /// Returns invalid data when the newest record's name is neither a string nor null.
    fn get_session_name(&self) -> std::io::Result<Option<&str>>;
}
