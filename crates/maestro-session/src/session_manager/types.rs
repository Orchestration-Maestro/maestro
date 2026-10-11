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
    /// Common identity and timestamp fields.
    #[serde(flatten)]
    pub base: SessionEntryBase,
    /// Recorded custom type.
    pub custom_type: String,
    /// Recorded data.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub data: Presence<T>,
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
    /// Common identity and timestamp fields.
    #[serde(flatten)]
    pub base: SessionEntryBase,
    /// Recorded custom type.
    pub custom_type: String,
    /// Recorded content.
    pub content: UserContent,
    /// Recorded details.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub details: Presence<T>,
    /// Recorded display.
    pub display: bool,
}
