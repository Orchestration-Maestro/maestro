//! Session file and context records shared by the native session and the extension guest.
use maestro_request::presence::{Omittable, Presence};
use maestro_request::types::UserContent;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Identity and timestamp shared by session entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEntryBase {
    /// Entry identity.
    pub id: String,
    /// Parent identity, or null for a root entry.
    pub parent_id: Option<String>,
    /// Entry timestamp text.
    pub timestamp: String,
}

/// The header line of a session file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type", rename = "session")]
pub struct SessionHeader {
    /// Format version; absent in first-version files.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub version: Presence<f64>,
    /// Session identity.
    pub id: String,
    /// Creation timestamp text.
    pub timestamp: String,
    /// Working directory.
    pub cwd: String,
    /// Session file this one was started from.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub parent_session: Presence<String>,
}

/// A conversation message stored as an entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename = "message", rename_all = "camelCase")]
pub struct SessionMessageEntry<M> {
    /// Entry identity and timestamp.
    #[serde(flatten)]
    pub base: SessionEntryBase,
    /// Stored message.
    pub message: M,
}

/// A thinking-level selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename = "thinking_level_change")]
#[serde(rename_all = "camelCase")]
pub struct ThinkingLevelChangeEntry {
    /// Entry identity and timestamp.
    #[serde(flatten)]
    pub base: SessionEntryBase,
    /// Selected thinking level text.
    pub thinking_level: String,
}

/// A provider and model selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename = "model_change", rename_all = "camelCase")]
pub struct ModelChangeEntry {
    /// Entry identity and timestamp.
    #[serde(flatten)]
    pub base: SessionEntryBase,
    /// Provider identity.
    pub provider: String,
    /// Model identity.
    pub model_id: String,
}

/// A compaction summary with the first entry it keeps.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename = "compaction", rename_all = "camelCase")]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
pub struct CompactionEntry<T = Value> {
    /// Entry identity and timestamp.
    #[serde(flatten)]
    pub base: SessionEntryBase,
    /// Summary text.
    pub summary: String,
    /// First entry kept after compaction.
    pub first_kept_entry_id: String,
    /// Context size before compaction.
    pub tokens_before: f64,
    /// Extension-defined details.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub details: Presence<T>,
    /// Whether an extension produced the summary.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub from_hook: Presence<bool>,
}

/// A summary of an abandoned branch.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename = "branch_summary", rename_all = "camelCase")]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
pub struct BranchSummaryEntry<T = Value> {
    /// Entry identity and timestamp.
    #[serde(flatten)]
    pub base: SessionEntryBase,
    /// Branch-point identifier, or `"root"` for a branch before the first entry.
    pub from_id: String,
    /// Summary text.
    pub summary: String,
    /// Extension-defined details.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub details: Presence<T>,
    /// Whether an extension produced the summary.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub from_hook: Presence<bool>,
}

/// Extension state that never enters model context.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename = "custom", rename_all = "camelCase")]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
pub struct CustomEntry<T = Value> {
    /// Extension-defined entry kind.
    pub custom_type: String,
    /// Extension-defined data.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub data: Presence<T>,
    /// Entry identity and timestamp.
    #[serde(flatten)]
    pub base: SessionEntryBase,
}

/// An extension message that joins model context.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename = "custom_message", rename_all = "camelCase")]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
pub struct CustomMessageEntry<T = Value> {
    /// Extension-defined entry kind.
    pub custom_type: String,
    /// Message content.
    pub content: UserContent,
    /// Whether the message is displayed.
    pub display: bool,
    /// Extension-defined details.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub details: Presence<T>,
    /// Entry identity and timestamp.
    #[serde(flatten)]
    pub base: SessionEntryBase,
}

/// A label change on an entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename = "label", rename_all = "camelCase")]
pub struct LabelEntry {
    /// Entry identity and timestamp.
    #[serde(flatten)]
    pub base: SessionEntryBase,
    /// Labelled entry.
    pub target_id: String,
    /// New label; absent clears it.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub label: Presence<String>,
}

/// Session metadata such as a display name.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename = "session_info", rename_all = "camelCase")]
pub struct SessionInfoEntry {
    /// Entry identity and timestamp.
    #[serde(flatten)]
    pub base: SessionEntryBase,
    /// Display name.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub name: Presence<String>,
}

/// Identity and parent metadata for a new session.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSessionOptions<P: Omittable> {
    /// Session identity; absent generates one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Session file the new session starts from.
    #[serde(default, skip_serializing_if = "Omittable::is_omitted")]
    pub parent_session: P,
}

/// The provider and model selected by a built context.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionContextModel<V: Omittable> {
    /// Provider identity.
    #[serde(skip_serializing_if = "Omittable::is_omitted")]
    pub provider: V,
    /// Model identity.
    #[serde(skip_serializing_if = "Omittable::is_omitted")]
    pub model_id: V,
}

/// Messages and settings selected for model context.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionContext<M, V: Omittable> {
    /// Messages in context order.
    pub messages: Vec<M>,
    /// Selected thinking level.
    #[serde(skip_serializing_if = "Omittable::is_omitted")]
    pub thinking_level: V,
    /// Selected model, if any.
    pub model: Option<SessionContextModel<V>>,
}

/// Where a fork starts relative to its entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ForkPosition {
    /// Before the entry.
    Before,
    /// At the entry.
    At,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Independently optional tree navigation choices.
pub struct NavigateTreeOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Whether to request summarization.
    pub summarize: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Literal optional instructions.
    pub custom_instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Whether to replace the host instructions.
    pub replace_instructions: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Literal optional label.
    pub label: Option<String>,
}
