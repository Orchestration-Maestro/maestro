#![doc = include_str!("../../../../docs/compaction.md")]
mod estimate;
pub use estimate::{
    calculate_context_tokens, estimate_context_tokens, estimate_tokens, get_last_assistant_usage,
    should_compact,
};
pub use maestro_entry::compaction::{CompactionSettings, FileOperations};
/// Library policy for automatic compaction and the retained window.
pub const DEFAULT_COMPACTION_SETTINGS: CompactionSettings = CompactionSettings {
    enabled: true,
    reserve_tokens: 16384.0,
    keep_recent_tokens: 20000.0,
};
/// Reported usage plus estimated messages following that usage.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextUsageEstimate {
    /// Combined context estimate.
    pub tokens: f64,
    /// Selected reported usage.
    pub usage_tokens: f64,
    /// Estimate after the selected usage, or the entire sequence without usage.
    pub trailing_tokens: f64,
    /// Position of the selected successful assistant usage.
    pub last_usage_index: Option<usize>,
}

mod cut;
pub use cut::{find_cut_point, find_turn_start_index};
/// Retained entry position and any split-turn prefix boundary.
#[derive(Debug)]
pub struct CutPointResult {
    /// First retained entry in the supplied range.
    pub first_kept_entry_index: usize,
    /// Turn start when the retained boundary is not a user message.
    pub turn_start_index: Option<usize>,
    /// Whether a turn start exists for a non-user cut.
    pub is_split_turn: bool,
}

/// Shared file tracking and conversation serialization.
pub mod utils;
pub use utils::{
    compute_file_lists, create_file_ops, extract_file_ops_from_message, format_file_operations,
    serialize_conversation,
};
/// File lists for summaries; [`compute_file_lists`] produces sorted, disjoint lists.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionDetails {
    /// Read paths not also edited or written.
    pub read_files: Vec<String>,
    /// Union of edited and written paths.
    pub modified_files: Vec<String>,
}
/// Owned raw-message snapshots prepared for a pending compaction.
pub type CompactionPreparation =
    maestro_entry::compaction::CompactionPreparation<serde_json::Value>;
mod prepare;
pub use prepare::prepare_compaction;
mod generation;
mod prompts;
pub use generation::{SummaryOptions, compact, generate_summary};
pub use maestro_entry::compaction::CompactionResult;

mod branch_summarization;
pub use branch_summarization::{CollectEntriesResult, collect_entries_for_branch_summary};

pub use branch_summarization::{BranchPreparation, BranchSummaryDetails, prepare_branch_entries};

pub use branch_summarization::{
    BranchSummaryRequest, BranchSummaryResult, GenerateBranchSummaryOptions,
    generate_branch_summary, generate_branch_summary_with,
};
pub use utils::SUMMARIZATION_SYSTEM_PROMPT;
