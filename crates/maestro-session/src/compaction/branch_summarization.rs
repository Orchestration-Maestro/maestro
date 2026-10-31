#![doc = include_str!("../../../../docs/branch_summary.md")]
use crate::SessionEntry;
use crate::session_manager::ReadonlySessionManager;
use serde_json::Value;
use std::collections::HashSet;
/// Borrowed abandoned entries and the deepest shared ancestor.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectEntriesResult<'a> {
    /// Chronological entries excluding the shared ancestor.
    pub entries: Vec<&'a SessionEntry>,
    /// Shared ancestor identity, if any.
    pub common_ancestor_id: Option<String>,
}
/// Collect the abandoned suffix using bounded session ancestry.
///
/// # Errors
/// Returns reached ancestry property errors without changing history.
pub fn collect_entries_for_branch_summary<'a>(
    session: &'a dyn ReadonlySessionManager,
    old_leaf_id: Option<&str>,
    target_id: &str,
) -> Result<CollectEntriesResult<'a>, serde_json::Error> {
    let Some(old) = old_leaf_id.filter(|id| !id.is_empty()) else {
        return Ok(CollectEntriesResult {
            entries: Vec::new(),
            common_ancestor_id: None,
        });
    };
    let old_path = session.get_branch(Some(old))?;
    let old_ids: HashSet<_> = old_path
        .iter()
        .filter_map(|entry| entry.get("id").and_then(Value::as_str))
        .collect();
    let target_path = session.get_branch(Some(target_id))?;
    let ancestor = target_path
        .iter()
        .rev()
        .filter_map(|entry| entry.get("id").and_then(Value::as_str))
        .find(|id| old_ids.contains(id));
    let start = ancestor
        .and_then(|id| {
            old_path
                .iter()
                .position(|entry| entry.get("id").and_then(Value::as_str) == Some(id))
        })
        .map_or(0, |position| position + 1);
    Ok(CollectEntriesResult {
        entries: old_path.into_iter().skip(start).collect(),
        common_ancestor_id: ancestor.map(str::to_owned),
    })
}

/// Stored built-in branch file details.
pub type BranchSummaryDetails = super::CompactionDetails;
/// Owned chronological raw messages and their accumulated token estimate.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchPreparation {
    /// Selected message snapshots.
    pub messages: Vec<Value>,
    /// Insertion-ordered file observations.
    pub file_ops: super::FileOperations,
    /// Estimated tokens, including an admitted oversized summary.
    pub total_tokens: f64,
}
/// Select only consumed entry fields, excluding tool results and metadata.
fn message(entry: &Value) -> Result<Option<Value>, serde_json::Error> {
    use crate::session_manager::context::{generated, property};
    Ok(
        match property(Some(entry), "type")?.and_then(Value::as_str) {
            Some("message") => {
                if property(entry.get("message"), "role")?.and_then(Value::as_str)
                    == Some("toolResult")
                {
                    None
                } else {
                    entry.get("message").cloned()
                }
            }
            Some("custom_message") => Some(generated(
                entry,
                "custom",
                &["customType", "content", "display", "details"],
            )),
            Some("branch_summary") => {
                Some(generated(entry, "branchSummary", &["summary", "fromId"]))
            }
            Some("compaction") => Some(generated(
                entry,
                "compactionSummary",
                &["summary", "tokensBefore"],
            )),
            _ => None,
        },
    )
}
/// Prepare the newest retained content, carrying all built-in branch file details first.
/// Nonpositive or omitted budgets are unlimited. A summary crossing the budget is
/// included only below ninety percent occupancy; either crossing stops traversal.
///
/// # Errors
/// Returns errors only for reached entry, estimate or file-array fields.
pub fn prepare_branch_entries(
    entries: &[SessionEntry],
    token_budget: Option<f64>,
) -> Result<BranchPreparation, serde_json::Error> {
    use crate::session_manager::context::property;
    let mut preparation = BranchPreparation {
        messages: Vec::new(),
        file_ops: super::create_file_ops(),
        total_tokens: 0.0,
    };
    for entry in entries {
        if property(Some(entry), "type")?.and_then(Value::as_str) == Some("branch_summary") {
            super::utils::carry_file_ops(entry, &mut preparation.file_ops)?;
        }
    }
    let budget = token_budget.unwrap_or(0.0);
    for entry in entries.iter().rev() {
        let Some(message) = message(entry)? else {
            continue;
        };
        super::extract_file_ops_from_message(&message, &mut preparation.file_ops);
        let tokens = super::estimate_tokens(&message)?;
        let crossing = budget > 0.0 && preparation.total_tokens + tokens > budget;
        if crossing
            && !(matches!(
                entry.get("type").and_then(Value::as_str),
                Some("compaction" | "branch_summary")
            ) && preparation.total_tokens < budget * 0.9)
        {
            break;
        }
        preparation.messages.push(message);
        preparation.total_tokens += tokens;
        if crossing {
            break;
        }
    }
    preparation.messages.reverse();
    Ok(preparation)
}

/// Exactly the reachable generated object shapes, without a wire discriminator.
#[derive(Debug, serde::Serialize)]
#[serde(untagged, rename_all_fields = "camelCase")]
pub enum BranchSummaryResult {
    /// Completed summary with tracked file lists.
    Summary {
        /// Preamble, generated text and optional file sections.
        summary: String,
        /// Read paths not modified.
        read_files: Vec<String>,
        /// Edited or written paths.
        modified_files: Vec<String>,
        /// Usage the provider reported for the request.
        usage: maestro_models::Usage,
    },
    /// Empty preparation; no request was made.
    NoContent {
        /// Explanation of absent content.
        summary: String,
    },
    /// Provider reported cancellation.
    Aborted {
        /// Cancellation marker.
        aborted: bool,
    },
    /// Provider reported a failed response.
    Error {
        /// Provider text or the summary failure fallback.
        error: String,
    },
}
/// Caller-owned controls for an abandoned-branch request.
pub struct GenerateBranchSummaryOptions {
    /// Explicit model selection.
    pub model: maestro_models::Model,
    /// Explicit authentication, including an empty key.
    pub api_key: String,
    /// Headers in supplied order.
    pub headers: Option<indexmap::IndexMap<String, String>>,
    /// Shared cancellation handle.
    pub signal: maestro_models::Cancellation,
    /// Optional focus or replacement, without trimming.
    pub custom_instructions: Option<String>,
    /// Whether nonempty custom instructions replace the branch prompt.
    pub replace_instructions: bool,
    /// Prompt/response reservation; omitted means 16384 tokens.
    pub reserve_tokens: Option<f64>,
}

/// Context preceding generated branch text.
const BRANCH_SUMMARY_PREAMBLE: &str = r"The user explored a different conversation branch before returning here.
Summary of that exploration:

";

/// Structured branch instructions.
const BRANCH_SUMMARY_PROMPT: &str = r#"Create a structured summary of this conversation branch for context when returning later.

Use this EXACT format:

## Goal
[What was the user trying to accomplish in this branch?]

## Constraints & Preferences
- [Any constraints, preferences, or requirements mentioned]
- [Or "(none)" if none were mentioned]

## Progress
### Done
- [x] [Completed tasks/changes]

### In Progress
- [ ] [Work that was started but not finished]

### Blocked
- [Issues preventing progress, if any]

## Key Decisions
- **[Decision]**: [Brief rationale]

## Next Steps
1. [What should happen next to continue this work]

Keep each section concise. Preserve exact file paths, function names, and error messages."#;

/// Start a branch summary eagerly and return an independently owned observation.
/// No history is mutated or saved. Projection errors precede invocation; adapter
/// failures propagate, while aborted/error responses have their own result shapes.
#[must_use]
pub fn generate_branch_summary(
    entries: &[SessionEntry],
    options: &GenerateBranchSummaryOptions,
) -> maestro_models::BoxFuture<Result<BranchSummaryResult, maestro_models::DiagnosticErrorInfo>> {
    let window = if options.model.context_window == 0.0 {
        128_000.0
    } else {
        options.model.context_window
    };
    let preparation = prepare_branch_entries(
        entries,
        Some(window - options.reserve_tokens.unwrap_or(16384.0)),
    );
    let preparation = match preparation {
        Ok(preparation) => preparation,
        Err(error) => return failed(error),
    };
    if preparation.messages.is_empty() {
        return Box::pin(async {
            Ok(BranchSummaryResult::NoContent {
                summary: "No content to summarize".into(),
            })
        });
    }
    let conversation = match super::utils::serialize_raw_conversation(&preparation.messages) {
        Ok(text) => text,
        Err(error) => return failed(error),
    };
    let instructions = match options
        .custom_instructions
        .as_deref()
        .filter(|text| !text.is_empty())
    {
        Some(custom) if options.replace_instructions => custom.to_owned(),
        Some(custom) => format!("{BRANCH_SUMMARY_PROMPT}\n\nAdditional focus: {custom}"),
        None => BRANCH_SUMMARY_PROMPT.into(),
    };
    let prompt = format!("<conversation>\n{conversation}\n</conversation>\n\n{instructions}");
    let completion = complete(prompt, options);
    let details = super::compute_file_lists(&preparation.file_ops);
    Box::pin(async move {
        let response = completion.await?;
        let response = response
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(result(&response, details))
    })
}
/// Preserve the existing native diagnostic boundary before asynchronous work.
fn failed(
    error: serde_json::Error,
) -> maestro_models::BoxFuture<Result<BranchSummaryResult, maestro_models::DiagnosticErrorInfo>> {
    Box::pin(async move {
        Err(maestro_models::DiagnosticErrorInfo::error(
            error.to_string(),
        ))
    })
}
/// Invoke the existing eager completion owner with branch-specific controls only.
fn complete(
    prompt: String,
    options: &GenerateBranchSummaryOptions,
) -> maestro_models::BoxFuture<
    Result<maestro_models::SharedAssistantMessage, maestro_models::DiagnosticErrorInfo>,
> {
    use maestro_models::{
        Context, Message, SimpleStreamOptions, StreamOptions, TextContent, UserBlock, UserContent,
        UserMessage,
    };
    maestro_models::complete_simple(
        options.model.clone(),
        Context {
            system_prompt: Some(super::utils::SUMMARIZATION_SYSTEM_PROMPT.into()),
            messages: vec![Message::User(UserMessage {
                content: UserContent::Blocks(vec![UserBlock::Text(TextContent::new(prompt))]),
                timestamp: maestro_models::timestamp_now(),
            })],
            tools: None,
        },
        Some(SimpleStreamOptions {
            common: StreamOptions {
                max_tokens: Some(2048.0),
                api_key: Some(options.api_key.clone()),
                headers: options.headers.clone(),
                signal: Some(options.signal.clone()),
                ..Default::default()
            },
            ..Default::default()
        }),
    )
}
/// Apply the branch response policy and append disjoint sorted file sections.
fn result(
    response: &maestro_models::AssistantMessage,
    details: BranchSummaryDetails,
) -> BranchSummaryResult {
    use maestro_models::{AssistantContent, StopReason};
    match response.stop_reason {
        StopReason::Aborted => BranchSummaryResult::Aborted { aborted: true },
        StopReason::Error => BranchSummaryResult::Error {
            error: response
                .error_message
                .as_deref()
                .filter(|text| !text.is_empty())
                .unwrap_or("Summarization failed")
                .to_owned(),
        },
        _ => {
            let text = response
                .content
                .iter()
                .filter_map(|block| match block {
                    AssistantContent::Text(text) => Some(text.text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            BranchSummaryResult::Summary {
                summary: format!(
                    "{BRANCH_SUMMARY_PREAMBLE}{text}{}",
                    super::format_file_operations(&details.read_files, &details.modified_files)
                ),
                read_files: details.read_files,
                modified_files: details.modified_files,
                usage: response.usage.clone(),
            }
        }
    }
}
