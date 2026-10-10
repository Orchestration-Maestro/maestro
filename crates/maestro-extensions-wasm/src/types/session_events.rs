//! Session and tree event records supplied to author handlers.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{AbortSignal, BranchSummaryEntry, CompactionEntry, Presence, SessionEntry};
use serde::{Deserialize, Serialize};
/// Notification containing the persisted compaction entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCompactEvent {
    /// Stored compaction entry.
    pub compaction_entry: CompactionEntry,
    /// Whether the compaction content was supplied by an extension hook.
    pub from_extension: bool,
}
/// Supplied branch positions and summary inputs for navigation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TreePreparation {
    /// Target entry identity.
    pub target_id: String,
    /// Previous leaf identity, or null.
    pub old_leaf_id: Option<String>,
    /// Common ancestor identity, or null.
    pub common_ancestor_id: Option<String>,
    /// Ordered entries supplied for summarization.
    pub entries_to_summarize: Vec<SessionEntry>,
    /// Whether the caller requested a summary.
    pub user_wants_summary: bool,
    /// Supplied custom instructions, null or omitted.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub custom_instructions: Presence<String>,
    /// Whether supplied instructions replace the defaults, null or omitted.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub replace_instructions: Presence<bool>,
    /// Supplied label, null or omitted.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub label: Presence<String>,
}
/// Notification containing old/new positions and an optional summary entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTreeEvent {
    /// New leaf identity, or null.
    pub new_leaf_id: Option<String>,
    /// Previous leaf identity, or null.
    pub old_leaf_id: Option<String>,
    /// Stored summary entry, null or omitted.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub summary_entry: Presence<BranchSummaryEntry>,
    /// Whether the tree summary content was supplied by an extension hook.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub from_extension: Presence<bool>,
}
/// Summary content supplied by a tree handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBeforeTreeSummary {
    /// Supplied summary content.
    pub summary: String,
    /// Opaque JSON text, carried without parsing or reformatting.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub details: Presence<String>,
}
/// Optional cancellation, summary and navigation-summary overrides.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBeforeTreeResult {
    /// Supplied cancellation decision, null or omitted.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub cancel: Presence<bool>,
    /// Supplied summary content.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub summary: Presence<SessionBeforeTreeSummary>,
    /// Supplied custom instructions, null or omitted.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub custom_instructions: Presence<String>,
    /// Whether supplied instructions replace the defaults, null or omitted.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub replace_instructions: Presence<bool>,
    /// Supplied label, null or omitted.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub label: Presence<String>,
}
/// Before-tree preparation with its owned cancellation signal.
#[derive(Debug)]
pub struct SessionBeforeTreeEvent {
    /// Supplied navigation preparation.
    pub preparation: TreePreparation,
    /// Owned cancellation signal, excluded from JSON.
    pub signal: AbortSignal,
}
/// Resource-free before-tree transport fields.
#[derive(Debug, Serialize, Deserialize)]
#[cfg(any(test, target_arch = "wasm32"))]
pub(crate) struct SessionBeforeTreeEventData {
    /// Supplied navigation preparation.
    pub(crate) preparation: TreePreparation,
}
