//! Full historical generation and result application.
use super::*;
/// Generate from the complete migrated history through the registered model consumer.
pub fn large_history(apply: bool) {
    use maestro_session::{
        DEFAULT_COMPACTION_SETTINGS, migrate_session_entries, parse_session_entries,
        prepare_compaction,
    };
    let case = corpus().large;
    let mut records =
        parse_session_entries(include_str!("../../fixtures/session_history_large.jsonl"));
    migrate_session_entries(&mut records).unwrap();
    let mut entries: Vec<_> = records
        .into_iter()
        .filter(|entry| entry["type"] != "session")
        .collect();
    let prepared = prepare_compaction(&entries, &DEFAULT_COMPACTION_SETTINGS)
        .unwrap()
        .unwrap();
    let mut history = default_response();
    history["content"][0]["text"] = json!(case.history);
    let mut prefix = default_response();
    prefix["content"][0]["text"] = json!(case.prefix);
    let adapter = adapter(json!({"history":history,"prefix":prefix}));
    let model = model(&corpus().rows[0], &adapter.api);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let result = runtime
        .block_on(compact(&prepared, &model, &options(&json!({}))))
        .unwrap();
    if apply {
        apply_result(&mut entries, &result, &case);
    } else {
        assert_eq!(result.summary, case.summary);
        assert_eq!(result.first_kept_entry_id, prepared.first_kept_entry_id);
        assert_eq!(result.tokens_before.to_bits(), 177_657.0_f64.to_bits());
        assert!(result.summary.len() > 100);
        assert_eq!(adapter.calls.lock().unwrap().len(), 2);
    }
}

/// Append the generated result and inspect the projected retained context.
fn apply_result(
    entries: &mut Vec<Value>,
    result: &maestro_session::CompactionResult<maestro_session::CompactionDetails>,
    case: &Large,
) {
    use maestro_session::build_session_context;
    let before = build_session_context(
        entries,
        Presence::Missing,
        None::<&std::collections::HashMap<_, _>>,
    )
    .unwrap();
    let mut entry = serde_json::to_value(result).unwrap();
    entry["type"] = json!("compaction");
    entry["id"] = json!("generated-compaction");
    entry["parentId"] = entries.last().unwrap()["id"].clone();
    entry["timestamp"] = json!("2024-03-09T16:00:00.123Z");
    entries.push(entry);
    let after = build_session_context(
        entries,
        Presence::Missing,
        None::<&std::collections::HashMap<_, _>>,
    )
    .unwrap();
    assert_eq!(before.messages.len(), case.before);
    assert_eq!(after.messages.len(), case.after);
    assert!(after.messages.len() < before.messages.len());
    assert_eq!(after.messages[0]["role"], "compactionSummary");
    assert_eq!(after.messages[0]["summary"], result.summary);
    assert_eq!(
        after.messages[1..],
        before.messages[before.messages.len() - after.messages.len() + 1..]
    );
}
