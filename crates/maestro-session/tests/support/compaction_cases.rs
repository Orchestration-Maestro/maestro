//! Recorded public compaction operations.
use maestro_session::*;
use serde::Deserialize;
use serde_json::{Value, json};
/// One recorded input and its complete observable output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Behavior group.
    group: String,
    /// Distinguishable input label.
    id: String,
    /// Public operation.
    operation: String,
    /// Supplied operands.
    args: Vec<Value>,
    /// Complete expected result.
    output: Output,
}
/// Success, absence or a reached native error.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum Output {
    /// Returned value.
    Value { value: Value },
    /// No result.
    Absent,
    /// Reached field error.
    Error,
}
/// Execute every recorded case in the named group.
pub fn group(name: &str) {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/compaction_cases.json")).unwrap();
    let selected: Vec<_> = cases.iter().filter(|case| case.group == name).collect();
    assert!(!selected.is_empty(), "{name}");
    for case in selected {
        let actual = operation(case);
        match &case.output {
            Output::Value { value } => {
                let actual = actual.unwrap().unwrap();
                super::value::assert_value(&actual, value, &case.id);
                assert_order(&actual, value, &case.id);
            }
            Output::Absent => assert!(actual.unwrap().is_none(), "{}", case.id),
            Output::Error => assert!(actual.is_err(), "{}", case.id),
        }
    }
}
/// Dispatch recorded input through the public operation.
fn operation(case: &Case) -> Result<Option<Value>, serde_json::Error> {
    let a = &case.args;
    let value = match case.operation.as_str() {
        "defaults-probe" => serde_json::to_value(DEFAULT_COMPACTION_SETTINGS)?,
        "shouldCompact" => json!(should_compact(
            a[0].as_f64().unwrap(),
            a[1].as_f64().unwrap(),
            &serde_json::from_value(a[2].clone())?
        )),
        "calculateContextTokens" => json!(calculate_context_tokens(&a[0])?),
        "getLastAssistantUsage" => {
            return Ok(get_last_assistant_usage(a[0].as_array().unwrap())?.cloned());
        }
        "estimateTokens" => json!(estimate_tokens(&a[0])?),
        "estimateContextTokens" => {
            serde_json::to_value(estimate_context_tokens(a[0].as_array().unwrap())?)?
        }
        "findTurnStartIndex" | "findCutPoint" => cut_operation(case)?,
        "extractFileOpsFromMessage" => {
            let mut ops = create_file_ops();
            extract_file_ops_from_message(&a[0], &mut ops);
            serde_json::to_value(ops)?
        }
        "computeFileLists" => {
            serde_json::to_value(compute_file_lists(&serde_json::from_value(a[0].clone())?))?
        }
        "prepareCompaction" => {
            return prepare_compaction(
                a[0].as_array().unwrap(),
                &serde_json::from_value(a[1].clone())?,
            )
            .map(|prepared| prepared.map(|prepared| serde_json::to_value(prepared).unwrap()));
        }
        "usage-negative-zero-probe" => {
            let usage: Value = serde_json::from_str(a[0].as_str().unwrap())?;
            let tokens = calculate_context_tokens(&usage)?;
            json!({"isNegativeZero": tokens == 0.0 && tokens.is_sign_negative()})
        }
        "formatFileOperations" => json!(format_file_operations(
            &serde_json::from_value::<Vec<String>>(a[0].clone())?,
            &serde_json::from_value::<Vec<String>>(a[1].clone())?
        )),
        "serializeConversation" => json!(serialize_conversation(&serde_json::from_value::<
            Vec<maestro_models::Message>,
        >(a[0].clone())?)?),
        "buildSessionContext" => serde_json::to_value(build_session_context(
            a[0].as_array().unwrap(),
            Presence::Missing,
            None::<&std::collections::HashMap<_, _>>,
        )?)?,
        "fixture-probe" => large_history()?,
        _ => panic!("unimplemented operation {}", case.operation),
    };
    Ok(Some(value))
}

/// Execute cut operations with native indices and the recorded absent-start sentinel.
fn cut_operation(case: &Case) -> Result<Value, serde_json::Error> {
    let a = &case.args;
    Ok(match case.operation.as_str() {
        "findTurnStartIndex" => json!(
            find_turn_start_index(
                a[0].as_array().unwrap(),
                a[1].as_u64().unwrap().try_into().unwrap(),
                a[2].as_u64().unwrap().try_into().unwrap()
            )?
            .map_or(-1, |index| i64::try_from(index).unwrap())
        ),
        "findCutPoint" => {
            let cut = find_cut_point(
                a[0].as_array().unwrap(),
                a[1].as_u64().unwrap().try_into().unwrap(),
                a[2].as_u64().unwrap().try_into().unwrap(),
                a[3].as_f64().unwrap(),
            )?;
            json!({"firstKeptEntryIndex": cut.first_kept_entry_index, "turnStartIndex": cut.turn_start_index.map_or(-1, |index| i64::try_from(index).unwrap()), "isSplitTurn": cut.is_split_turn})
        }
        _ => unreachable!(),
    })
}

/// Parse the complete reusable historical file through its production conversion owner.
fn large_history() -> Result<Value, serde_json::Error> {
    let mut records =
        parse_session_entries(include_str!("../fixtures/session_history_large.jsonl"));
    let parsed_records = records.len();
    migrate_session_entries(&mut records).unwrap();
    let entries: Vec<_> = records
        .into_iter()
        .filter(|entry| entry["type"] != "session")
        .collect();
    let cut = find_cut_point(
        &entries,
        0,
        entries.len(),
        DEFAULT_COMPACTION_SETTINGS.keep_recent_tokens,
    )?;
    let context = build_session_context(
        &entries,
        Presence::Missing,
        None::<&std::collections::HashMap<_, _>>,
    )?;
    let prepared = prepare_compaction(&entries, &DEFAULT_COMPACTION_SETTINGS)?.unwrap();
    assert_eq!(
        entries[cut.first_kept_entry_index]["id"],
        prepared.first_kept_entry_id
    );
    assert!(
        entries
            .iter()
            .all(|entry| entry["id"].as_str().is_some_and(|id| !id.is_empty()))
    );
    for pair in entries.windows(2) {
        assert_eq!(pair[1]["parentId"], pair[0]["id"]);
    }
    Ok(json!({
        "parsedRecords": parsed_records, "branchEntries": entries.len(),
        "messageEntries": entries.iter().filter(|entry| entry["type"] == "message").count(),
        "contextMessages": context.messages.len(), "model": context.model,
        "cut": {"firstKeptEntryIndex": cut.first_kept_entry_index, "turnStartIndex": cut.turn_start_index, "isSplitTurn": cut.is_split_turn},
        "cutRole": entries[cut.first_kept_entry_index]["message"]["role"],
        "preparedTokens": prepared.tokens_before, "preparedHistoryMessages": prepared.messages_to_summarize.len(),
        "preparedPrefixMessages": prepared.turn_prefix_messages.len()
    }))
}

/// Check object key sequences, including nested raw messages and ordered arguments.
fn assert_order(actual: &Value, expected: &Value, label: &str) {
    match (actual, expected) {
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(
                actual.keys().collect::<Vec<_>>(),
                expected.keys().collect::<Vec<_>>(),
                "{label}"
            );
            for (key, value) in expected {
                assert_order(&actual[key], value, label);
            }
        }
        (Value::Array(actual), Value::Array(expected)) => {
            for (actual, expected) in actual.iter().zip(expected) {
                assert_order(actual, expected, label);
            }
        }
        _ => {}
    }
}
