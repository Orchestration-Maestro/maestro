//! Session and tree event records supplied to author handlers.
use crate::presence::Presence;
use crate::types::Usage;
use serde::{Deserialize, Serialize};
/// Notification containing the persisted compaction entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCompactEvent<C> {
    /// Stored compaction entry.
    pub compaction_entry: C,
    /// Whether the compaction content was supplied by an extension hook.
    pub from_extension: bool,
}
/// Supplied branch positions and summary inputs for navigation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TreePreparation<E> {
    /// Target entry identity.
    pub target_id: String,
    /// Previous leaf identity, or null.
    pub old_leaf_id: Option<String>,
    /// Common ancestor identity, or null.
    pub common_ancestor_id: Option<String>,
    /// Ordered entries supplied for summarization.
    pub entries_to_summarize: Vec<E>,
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
#[serde(bound(deserialize = "B: Deserialize<'de>"))]
pub struct SessionTreeEvent<B> {
    /// New leaf identity, or null.
    pub new_leaf_id: Option<String>,
    /// Previous leaf identity, or null.
    pub old_leaf_id: Option<String>,
    /// Stored summary entry, null or omitted.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub summary_entry: Presence<B>,
    /// Whether the tree summary content was supplied by an extension hook.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub from_extension: Presence<bool>,
}
/// Summary content supplied by a tree handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound(deserialize = "D: Deserialize<'de>"))]
pub struct SessionBeforeTreeSummary<D> {
    /// Supplied summary content.
    pub summary: String,
    /// Opaque details in the caller's representation.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub details: Presence<D>,
    /// Usage of the model calls that produced the summary.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub usage: Presence<Usage>,
}
/// Optional cancellation, summary and navigation-summary overrides.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound(deserialize = "D: Deserialize<'de>"))]
pub struct SessionBeforeTreeResult<D> {
    /// Supplied cancellation decision, null or omitted.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub cancel: Presence<bool>,
    /// Supplied summary content.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub summary: Presence<SessionBeforeTreeSummary<D>>,
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
pub struct SessionBeforeTreeEvent<P, S> {
    /// Supplied navigation preparation.
    pub preparation: P,
    /// Owned cancellation signal, excluded from JSON.
    pub signal: S,
}
