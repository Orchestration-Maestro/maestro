//! Context estimates, retained cuts and summary input through the public compaction interface.
#![cfg(test)]
use maestro_session::{
    CompactionSettings, DEFAULT_COMPACTION_SETTINGS, estimate_context_tokens, estimate_tokens,
    find_cut_point, find_turn_start_index, get_last_assistant_usage, prepare_branch_entries,
    prepare_compaction, serialize_conversation,
};
use serde_json::{Value, json};

/// One image attachment record.
fn image() -> Value {
    json!({"type": "image", "data": "AAAA", "mimeType": "image/png"})
}

/// Assert two token counts are the same double.
#[track_caller]
fn assert_tokens(actual: f64, expected: f64) {
    assert_eq!(actual.to_bits(), expected.to_bits());
}

/// A saved usage reporting `total` context tokens.
fn usage(total: u64) -> Value {
    json!({"input": total, "output": 0, "cacheRead": 0, "cacheWrite": 0, "totalTokens": total,
        "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0}})
}

/// A finished assistant message with the supplied usage and timestamp.
fn assistant(text: &str, usage: &Value, timestamp: u64) -> Value {
    json!({"role": "assistant", "content": [{"type": "text", "text": text}], "api": "a",
        "provider": "p", "model": "m", "usage": usage, "stopReason": "stop", "timestamp": timestamp})
}

/// A user message with text content.
fn user(text: &str, timestamp: u64) -> Value {
    json!({"role": "user", "content": text, "timestamp": timestamp})
}

/// A stored message entry.
fn message_entry(id: &str, message: &Value) -> Value {
    json!({"type": "message", "id": id, "parentId": null, "timestamp": "2026-01-01T00:00:00.000Z",
        "message": message})
}

/// A stored custom-message entry, which joins model context.
fn custom_entry(id: &str, text: &str) -> Value {
    json!({"type": "custom_message", "id": id, "parentId": null,
        "timestamp": "2026-01-01T00:00:00.000Z", "customType": "note", "content": text,
        "display": true})
}

/// A stored branch-summary entry.
fn branch_summary_entry(id: &str, summary: &str) -> Value {
    json!({"type": "branch_summary", "id": id, "parentId": null,
        "timestamp": "2026-01-01T00:00:00.000Z", "fromId": "root", "summary": summary})
}

#[test]
fn user_image_attachment_weighs_like_a_tool_result_image() {
    let attached = json!({"role": "user", "content": [image()], "timestamp": 1});
    let returned = json!({"role": "toolResult", "toolCallId": "t", "toolName": "read",
        "content": [image()], "isError": false, "timestamp": 1});
    assert_tokens(estimate_tokens(&returned).unwrap(), 1200.0);
    assert_tokens(estimate_tokens(&attached).unwrap(), 1200.0);
    let captioned = json!({"role": "user", "timestamp": 1,
        "content": [{"type": "text", "text": "abcd"}, image()]});
    assert_tokens(estimate_tokens(&captioned).unwrap(), 1201.0);
}

#[test]
fn all_zero_assistant_usage_falls_back_to_the_previous_real_usage() {
    let zero = usage(0);
    let messages = [
        user("first", 1),
        assistant("answer", &usage(900), 2),
        user("again", 3),
        assistant("partial", &zero, 4),
    ];
    let estimate = estimate_context_tokens(&messages).unwrap();
    assert_eq!(estimate.last_usage_index, Some(1));
    assert_tokens(estimate.usage_tokens, 900.0);
    let trailing = estimate_tokens(&messages[2]).unwrap() + estimate_tokens(&messages[3]).unwrap();
    assert_tokens(estimate.trailing_tokens, trailing);

    let only_zero = estimate_context_tokens(&[user("first", 1), assistant("partial", &zero, 2)]);
    let only_zero = only_zero.unwrap();
    assert_eq!(only_zero.last_usage_index, None);
    assert_tokens(only_zero.usage_tokens, 0.0);
    assert_tokens(only_zero.tokens, 4.0);

    let entries = [
        message_entry("a", &assistant("answer", &usage(900), 2)),
        message_entry("b", &assistant("partial", &zero, 4)),
    ];
    assert_eq!(
        get_last_assistant_usage(&entries).unwrap(),
        Some(&usage(900))
    );
}

#[test]
fn usage_older_than_a_compaction_summary_is_ignored() {
    let summary = json!({"role": "compactionSummary", "summary": "abcdefgh", "tokensBefore": 9000,
        "timestamp": 100});
    let stale = assistant("kept", &usage(9000), 50);
    let before_reply =
        estimate_context_tokens(&[summary.clone(), user("kept question", 40), stale.clone()])
            .unwrap();
    assert_eq!(before_reply.last_usage_index, None);
    assert_tokens(before_reply.usage_tokens, 0.0);
    assert_tokens(before_reply.tokens, 2.0 + 4.0 + 1.0);

    let fresh = assistant("new", &usage(300), 120);
    let after_reply = estimate_context_tokens(&[
        summary,
        user("kept question", 40),
        stale,
        user("next", 110),
        fresh,
    ])
    .unwrap();
    assert_eq!(after_reply.last_usage_index, Some(4));
    assert_tokens(after_reply.tokens, 300.0);
}

#[test]
fn stale_usage_does_not_inflate_tokens_before_a_second_compaction() {
    let chain = |id: &str, parent: Option<&str>, mut entry: Value| {
        entry["id"] = json!(id);
        entry["parentId"] = json!(parent);
        entry
    };
    let entries = [
        chain("u1", None, message_entry("", &user("old question", 1))),
        chain(
            "a1",
            Some("u1"),
            message_entry("", &assistant("old answer", &usage(90_000), 2)),
        ),
        chain(
            "c1",
            Some("a1"),
            json!({"type": "compaction", "timestamp": "2026-01-01T00:00:01.000Z",
                "summary": "abcdefgh", "firstKeptEntryId": "a1", "tokensBefore": 90_000}),
        ),
        chain(
            "u2",
            Some("c1"),
            message_entry("", &user("new question", 3)),
        ),
    ];
    let settings = CompactionSettings {
        keep_recent_tokens: 1.0,
        ..DEFAULT_COMPACTION_SETTINGS
    };
    let prepared = prepare_compaction(&entries, &settings).unwrap().unwrap();
    assert!(prepared.tokens_before < 100.0, "{}", prepared.tokens_before);
}

#[test]
fn custom_messages_count_toward_the_kept_budget_and_start_turns() {
    let entries = [
        message_entry("u", &user("hi", 1)),
        message_entry("a1", &assistant("hello", &usage(1), 2)),
        custom_entry("c", &"x".repeat(4000)),
        message_entry("a2", &assistant("ok", &usage(1), 3)),
    ];
    let tiny = find_cut_point(&entries, 0, entries.len(), 1.0).unwrap();
    assert_eq!(tiny.first_kept_entry_index, 3);
    assert_eq!(tiny.turn_start_index, Some(2));
    assert!(tiny.is_split_turn);
    let fits = find_cut_point(&entries, 0, entries.len(), 2.0).unwrap();
    assert_eq!(fits.first_kept_entry_index, 2);
    assert_eq!(fits.turn_start_index, None);
    assert!(!fits.is_split_turn);
}

#[test]
fn summary_entries_count_toward_the_kept_budget() {
    let entries = [
        message_entry("u", &user("hi", 1)),
        message_entry("a1", &assistant("hello", &usage(1), 2)),
        branch_summary_entry("b", &"y".repeat(4000)),
        message_entry("a2", &assistant("ok", &usage(1), 3)),
    ];
    let fits = find_cut_point(&entries, 0, entries.len(), 2.0).unwrap();
    assert_eq!(fits.first_kept_entry_index, 2);
    assert!(!fits.is_split_turn);
}

#[test]
fn empty_branch_summaries_are_neither_cuts_nor_turn_starts() {
    let entries = [
        message_entry("u", &user("hi", 1)),
        branch_summary_entry("b", ""),
        message_entry("a", &assistant("ok", &usage(1), 2)),
    ];
    assert_eq!(find_turn_start_index(&entries, 2, 0).unwrap(), Some(0));
    let cut = find_cut_point(&entries, 0, entries.len(), 1.0).unwrap();
    assert_eq!(cut.first_kept_entry_index, 1);
    assert_eq!(cut.turn_start_index, Some(0));
}

#[test]
fn a_zero_budget_crosses_at_the_newest_message_even_when_it_weighs_nothing() {
    let entries = [
        message_entry("u1", &user("hi", 1)),
        message_entry("a", &assistant("ok", &usage(1), 2)),
        message_entry("u2", &user("", 3)),
    ];
    let cut = find_cut_point(&entries, 0, entries.len(), 0.0).unwrap();
    assert_eq!(cut.first_kept_entry_index, 2);
    assert_eq!(cut.turn_start_index, None);
    assert!(!cut.is_split_turn);
}

#[test]
fn summary_input_omits_an_empty_branch_summary() {
    let entries = [
        message_entry("u1", &user("hi", 1)),
        branch_summary_entry("b", ""),
        message_entry("a1", &assistant("hello", &usage(1), 2)),
        message_entry("u2", &user("again", 3)),
        message_entry("a2", &assistant("ok", &usage(1), 4)),
    ];
    let settings = CompactionSettings {
        keep_recent_tokens: 1.0,
        ..DEFAULT_COMPACTION_SETTINGS
    };
    let prepared = prepare_compaction(&entries, &settings).unwrap().unwrap();
    let roles: Vec<_> = prepared
        .messages_to_summarize
        .iter()
        .map(|message| message["role"].as_str().unwrap())
        .collect();
    assert_eq!(roles, ["user", "assistant"]);
}

#[test]
fn branch_preparation_omits_an_empty_branch_summary() {
    let entries = [
        message_entry("u", &user("hi", 1)),
        branch_summary_entry("b", ""),
        custom_entry("c", "note"),
    ];
    let prepared = prepare_branch_entries(&entries, None).unwrap();
    let roles: Vec<_> = prepared
        .messages
        .iter()
        .map(|message| message["role"].as_str().unwrap())
        .collect();
    assert_eq!(roles, ["user", "custom"]);
}

#[test]
fn generated_message_roles_start_turns() {
    for role in [
        json!({"role": "custom", "customType": "n", "content": "note", "display": true,
            "timestamp": 2}),
        json!({"role": "branchSummary", "summary": "s", "fromId": "x", "timestamp": 2}),
        json!({"role": "compactionSummary", "summary": "s", "tokensBefore": 1, "timestamp": 2}),
        json!({"role": "bashExecution", "command": "ls", "output": "a", "exitCode": 0,
            "cancelled": false, "truncated": false, "timestamp": 2}),
    ] {
        let entries = [
            message_entry("u", &user("hi", 1)),
            message_entry("g", &role),
            message_entry("a", &assistant("ok", &usage(1), 3)),
        ];
        assert_eq!(
            find_turn_start_index(&entries, 2, 0).unwrap(),
            Some(1),
            "{role}"
        );
        let cut = find_cut_point(&entries, 0, entries.len(), 1.0).unwrap();
        assert_eq!(cut.first_kept_entry_index, 2, "{role}");
        assert_eq!(cut.turn_start_index, Some(1), "{role}");
        assert!(cut.is_split_turn, "{role}");
        let starts = find_cut_point(&entries, 0, entries.len(), 2.0).unwrap();
        assert_eq!(starts.first_kept_entry_index, 1, "{role}");
        assert_eq!(starts.turn_start_index, None, "{role}");
        assert!(!starts.is_split_turn, "{role}");
    }
}

#[test]
fn conversation_serialization_leaves_thinking_out() {
    let messages: Vec<maestro_models::Message> = serde_json::from_value(json!([
        {"role": "user", "content": "go", "timestamp": 1},
        {"role": "assistant", "api": "a", "provider": "p", "model": "m", "timestamp": 2,
            "usage": usage(1), "stopReason": "stop",
            "content": [
                {"type": "thinking", "thinking": "private reasoning"},
                {"type": "text", "text": "visible"},
                {"type": "toolCall", "id": "t", "name": "read", "arguments": {"path": "a"}}
            ]}
    ]))
    .unwrap();
    assert_eq!(
        serialize_conversation(&messages).unwrap(),
        "[User]: go\n\n[Assistant]: visible\n\n[Assistant tool calls]: read(path=\"a\")"
    );
}
