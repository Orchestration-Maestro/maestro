//! Summary generation reports the provider usage beside its text.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support::scripted_summary::{ScriptedSummary, options, usage};
use maestro_session::{generate_summary, generate_summary_with_usage};
use serde_json::json;

/// Complete `future` on a fresh current-thread runtime.
fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(future)
}

#[test]
fn summary_with_usage_returns_text_and_provider_usage() {
    let reported = usage(11, 7, 0.5);
    let (provider, model) = ScriptedSummary::register(vec![("## Goal\nSummary", reported.clone())]);
    let messages = [json!({"role": "user", "content": "work", "timestamp": 1})];
    let summary = block_on(generate_summary_with_usage(
        &messages,
        &model,
        2000.0,
        None,
        &options(),
    ))
    .unwrap();
    assert_eq!(summary.text, "## Goal\nSummary");
    let expected: maestro_models::Usage = serde_json::from_value(reported).unwrap();
    assert_eq!(summary.usage, expected);
    assert_eq!(provider.remaining(), 0);
}

#[test]
fn text_only_summary_keeps_returning_text() {
    let (_provider, model) =
        ScriptedSummary::register(vec![("## Goal\nSummary", usage(11, 7, 0.5))]);
    let messages = [json!({"role": "user", "content": "work", "timestamp": 1})];
    let text = block_on(generate_summary(
        &messages,
        &model,
        2000.0,
        None,
        &options(),
    ))
    .unwrap();
    assert_eq!(text, "## Goal\nSummary");
}

/// A preparation summarizing `history`, with `prefix` as the early part of a split turn.
fn preparation(history: &[&str], prefix: &[&str]) -> maestro_session::CompactionPreparation {
    let messages = |texts: &[&str]| -> Vec<serde_json::Value> {
        texts
            .iter()
            .map(|text| json!({"role": "user", "content": text, "timestamp": 1}))
            .collect()
    };
    maestro_session::CompactionPreparation {
        first_kept_entry_id: "kept".into(),
        messages_to_summarize: messages(history),
        turn_prefix_messages: messages(prefix),
        is_split_turn: !prefix.is_empty(),
        tokens_before: 100.0,
        previous_summary: maestro_session::Presence::Missing,
        file_ops: maestro_session::create_file_ops(),
        settings: maestro_session::DEFAULT_COMPACTION_SETTINGS,
    }
}

/// The usage a compaction result carries, as the stored JSON.
fn result_usage(
    result: &maestro_session::CompactionResult<maestro_session::compaction::CompactionDetails>,
) -> serde_json::Value {
    serde_json::to_value(result).unwrap()["usage"].clone()
}

#[test]
fn compaction_reports_the_usage_of_its_single_request() {
    let (_provider, model) = ScriptedSummary::register(vec![("history", usage(11, 7, 0.5))]);
    let result = block_on(maestro_session::compact(
        &preparation(&["work"], &[]),
        &model,
        &options(),
    ))
    .unwrap();
    assert_eq!(result_usage(&result), usage_as_written(11, 7, 0.5));
}

#[test]
fn split_turn_compaction_reports_the_combined_usage_of_both_requests() {
    let (_provider, model) = ScriptedSummary::register(vec![
        ("history", usage(11, 7, 0.5)),
        ("prefix", usage(20, 5, 0.25)),
    ]);
    let result = block_on(maestro_session::compact(
        &preparation(&["work"], &["start"]),
        &model,
        &options(),
    ))
    .unwrap();
    assert_eq!(result_usage(&result), usage_as_written(31, 12, 0.75));
}

#[test]
fn split_turn_without_history_reports_only_the_prefix_usage() {
    let (provider, model) = ScriptedSummary::register(vec![("prefix", usage(20, 5, 0.25))]);
    let result = block_on(maestro_session::compact(
        &preparation(&[], &["start"]),
        &model,
        &options(),
    ))
    .unwrap();
    assert_eq!(result_usage(&result), usage_as_written(20, 5, 0.25));
    assert_eq!(provider.remaining(), 0);
}

#[test]
fn branch_summary_reports_the_usage_of_its_request() {
    let (_provider, model) = ScriptedSummary::register(vec![("branch text", usage(40, 8, 0.75))]);
    let options = maestro_session::GenerateBranchSummaryOptions {
        model,
        api_key: String::new(),
        headers: None,
        signal: maestro_models::Cancellation::new(),
        custom_instructions: None,
        replace_instructions: false,
        reserve_tokens: None,
    };
    let entries = [json!({"type": "message", "id": "a", "parentId": null,
        "message": {"role": "user", "content": "explore", "timestamp": 1}})];
    let result = block_on(maestro_session::generate_branch_summary(&entries, &options)).unwrap();
    assert_eq!(
        serde_json::to_value(&result).unwrap()["usage"],
        usage_as_written(40, 8, 0.75)
    );
}

/// The usage record as a typed owner writes it back: whole numbers as doubles.
fn usage_as_written(input: u64, output: u64, cost: f64) -> serde_json::Value {
    let typed: maestro_models::Usage = serde_json::from_value(usage(input, output, cost)).unwrap();
    serde_json::to_value(typed).unwrap()
}
