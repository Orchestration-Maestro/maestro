//! Raw entries and explicit session record views.
use maestro_models::UserContent;
use maestro_request::presence::Presence;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Unvalidated session entry.
pub type SessionEntry = Value;
/// Surviving model observations.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionContextModel {
    /// Observed provider.
    #[serde(skip_serializing_if = "Presence::is_missing")]
    pub provider: Presence<Value>,
    /// Observed model identifier.
    #[serde(skip_serializing_if = "Presence::is_missing")]
    pub model_id: Presence<Value>,
}
/// Owned context values; typed messages are explicit later reads.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionContext {
    /// Selected message values.
    pub messages: Vec<Value>,
    /// Last observed thinking level.
    #[serde(skip_serializing_if = "Presence::is_missing")]
    pub thinking_level: Presence<Value>,
    /// Last observed model.
    pub model: Option<SessionContextModel>,
}
/// Unvalidated parsed file value.
pub type FileEntry = Value;

/// Current session format version.
pub const CURRENT_SESSION_VERSION: u32 = 3;

/// Explicit session record view.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type", rename = "session")]
pub struct SessionHeader {
    /// Recorded version.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub version: Presence<f64>,
    /// Recorded id.
    pub id: String,
    /// Recorded timestamp.
    pub timestamp: String,
    /// Recorded cwd.
    pub cwd: String,
    /// Recorded parent session.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub parent_session: Presence<String>,
}

/// Explicit common entry fields.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEntryBase {
    /// Recorded id.
    pub id: String,
    /// Recorded parent id.
    pub parent_id: Option<String>,
    /// Recorded timestamp.
    pub timestamp: String,
}

/// Explicit message record view.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type", rename = "message")]
pub struct SessionMessageEntry {
    /// Common identity and timestamp fields.
    #[serde(flatten)]
    pub base: SessionEntryBase,
    /// Recorded message.
    pub message: Value,
}

/// Explicit thinking level change record view.
#[derive(Debug, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    tag = "type",
    rename = "thinking_level_change"
)]
pub struct ThinkingLevelChangeEntry {
    /// Common identity and timestamp fields.
    #[serde(flatten)]
    pub base: SessionEntryBase,
    /// Recorded thinking level.
    pub thinking_level: String,
}

/// Explicit model change record view.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type", rename = "model_change")]
pub struct ModelChangeEntry {
    /// Common identity and timestamp fields.
    #[serde(flatten)]
    pub base: SessionEntryBase,
    /// Recorded provider.
    pub provider: String,
    /// Recorded model id.
    pub model_id: String,
}

/// Explicit compaction record view.
#[derive(Debug, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    tag = "type",
    rename = "compaction",
    bound(deserialize = "T: Deserialize<'de>")
)]
pub struct CompactionEntry<T = Value> {
    /// Common identity and timestamp fields.
    #[serde(flatten)]
    pub base: SessionEntryBase,
    /// Recorded summary.
    pub summary: String,
    /// Recorded first kept entry id.
    pub first_kept_entry_id: String,
    /// Recorded tokens before.
    pub tokens_before: f64,
    /// Recorded details.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub details: Presence<T>,
    /// Recorded from hook.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub from_hook: Presence<bool>,
}

/// Explicit branch summary record view.
#[derive(Debug, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    tag = "type",
    rename = "branch_summary",
    bound(deserialize = "T: Deserialize<'de>")
)]
pub struct BranchSummaryEntry<T = Value> {
    /// Common identity and timestamp fields.
    #[serde(flatten)]
    pub base: SessionEntryBase,
    /// Recorded from id.
    pub from_id: String,
    /// Recorded summary.
    pub summary: String,
    /// Recorded details.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub details: Presence<T>,
    /// Recorded from hook.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub from_hook: Presence<bool>,
}

/// Explicit custom record view.
#[derive(Debug, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    tag = "type",
    rename = "custom",
    bound(deserialize = "T: Deserialize<'de>")
)]
pub struct CustomEntry<T = Value> {
    /// Recorded custom type.
    pub custom_type: String,
    /// Recorded data.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub data: Presence<T>,
    /// Common identity and timestamp fields.
    #[serde(flatten)]
    pub base: SessionEntryBase,
}

/// Explicit label record view.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type", rename = "label")]
pub struct LabelEntry {
    /// Common identity and timestamp fields.
    #[serde(flatten)]
    pub base: SessionEntryBase,
    /// Recorded target id.
    pub target_id: String,
    /// Recorded label.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub label: Presence<String>,
}

/// Explicit session info record view.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type", rename = "session_info")]
pub struct SessionInfoEntry {
    /// Common identity and timestamp fields.
    #[serde(flatten)]
    pub base: SessionEntryBase,
    /// Recorded name.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub name: Presence<String>,
}

/// Explicit custom message record view.
#[derive(Debug, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    tag = "type",
    rename = "custom_message",
    bound(deserialize = "T: Deserialize<'de>")
)]
pub struct CustomMessageEntry<T = Value> {
    /// Recorded custom type.
    pub custom_type: String,
    /// Recorded content.
    pub content: UserContent,
    /// Recorded display.
    pub display: bool,
    /// Recorded details.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub details: Presence<T>,
    /// Common identity and timestamp fields.
    #[serde(flatten)]
    pub base: SessionEntryBase,
}

/// Identity and parent metadata for a new session.
#[derive(Debug, Default)]
pub struct NewSessionOptions {
    /// Explicit identity, including an empty string; absence generates an identity.
    pub id: Option<String>,
    /// Optional parent-session metadata.
    pub parent_session: Presence<String>,
}

/// Supplied optional operands of a compaction record; both are omitted by default.
#[derive(Debug, Default)]
pub struct CompactionOptions {
    /// Extension-specific details.
    pub details: Presence<Value>,
    /// Whether an extension generated the summary.
    pub from_hook: Presence<bool>,
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
    /// Timestamp of the latest label change, if any; an explicit null stays null.
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
