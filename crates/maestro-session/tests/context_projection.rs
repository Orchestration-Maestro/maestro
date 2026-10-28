//! Supplied-entry context projection witnesses.
#![cfg(test)]
use maestro_request::presence::Presence;
use serde::Deserialize;

#[test]
fn empty_history_has_default_context() {
    support::session_context::group("empty_history_has_default_context");
}
use crate::support;
#[test]
fn one_user_entry_becomes_context() {
    support::session_context::group("one_user_entry_becomes_context");
}
#[test]
fn thinking_changes_select_latest_value() {
    support::session_context::group("thinking_changes_select_latest_value");
}
#[test]
fn compaction_precedes_kept_and_later_messages() {
    support::session_context::group("compaction_precedes_kept_and_later_messages");
}
#[test]
fn branch_summary_occupies_its_path_position() {
    support::session_context::group("branch_summary_occupies_its_path_position");
}
#[test]
fn generated_messages_keep_raw_fields() {
    support::session_context::group("generated_messages_keep_raw_fields");
}

#[test]
fn jsonl_framing_keeps_accepted_values() {
    support::session_context::group("jsonl_framing_keeps_accepted_values");
}

#[test]
fn latest_compaction_uses_reverse_input_order() {
    support::session_context::group("latest_compaction_uses_reverse_input_order");
}

#[test]
fn conversation_keeps_message_order() {
    support::session_context::group("conversation_keeps_message_order");
}

#[test]
fn assistant_selects_recorded_model() {
    support::session_context::group("assistant_selects_recorded_model");
}

#[test]
fn assistant_overrides_model_selection() {
    support::session_context::group("assistant_overrides_model_selection");
}

#[test]
fn compaction_can_keep_the_first_entry() {
    support::session_context::group("compaction_can_keep_the_first_entry");
}

#[test]
fn newest_path_compaction_wins() {
    support::session_context::group("newest_path_compaction_wins");
}

#[test]
fn leaf_selection_chooses_its_ancestry() {
    support::session_context::group("leaf_selection_chooses_its_ancestry");
}

#[test]
fn branches_keep_independent_compaction_paths() {
    support::session_context::group("branches_keep_independent_compaction_paths");
}

#[test]
fn unknown_leaf_uses_last_entry() {
    support::session_context::group("unknown_leaf_uses_last_entry");
}

#[test]
fn orphan_keeps_its_reachable_suffix() {
    support::session_context::group("orphan_keeps_its_reachable_suffix");
}

#[test]
fn raw_records_keep_order_duplicates_and_presence() {
    support::session_context::group("raw_records_keep_order_duplicates_and_presence");
}

#[test]
fn leaf_presence_controls_empty_and_fallback() {
    support::session_context::group("leaf_presence_controls_empty_and_fallback");
}

#[test]
fn supplied_index_is_authoritative() {
    support::session_context::group("supplied_index_is_authoritative");
}

#[test]
fn duplicate_ids_keep_last_lookup() {
    support::session_context::group("duplicate_ids_keep_last_lookup");
}

#[test]
fn cycles_terminate_without_dropping_finite_paths() {
    support::session_context::group("cycles_terminate_without_dropping_finite_paths");
}

#[test]
fn settings_observe_the_entire_selected_path() {
    support::session_context::group("settings_observe_the_entire_selected_path");
}

#[test]
fn precompaction_model_change_survives_without_overwrite() {
    support::session_context::group("precompaction_model_change_survives_without_overwrite");
}

#[test]
fn kept_range_uses_the_precompaction_path() {
    support::session_context::group("kept_range_uses_the_precompaction_path");
}

#[test]
fn custom_messages_and_summaries_select_only_message_kinds() {
    support::session_context::group("custom_messages_and_summaries_select_only_message_kinds");
}

#[test]
fn projection_defers_typed_message_reads() {
    support::session_context::group("projection_defers_typed_message_reads");
    for case in support::session_context::cases()
        .into_iter()
        .filter(|case| {
            matches!(
                case.id.as_str(),
                "future-content" | "raw-user-content" | "unknown-role"
            )
        })
    {
        let context = support::session_context::context(&case.args).unwrap();
        let message = &context.messages[0];
        if case.id == "future-content" {
            assert!(
                maestro_request::messages::CustomMessage::<serde_json::Value>::deserialize(message)
                    .is_err()
            );
        } else {
            assert!(maestro_models::Message::deserialize(message).is_err());
        }
    }
}

#[test]
fn settings_keep_raw_values_until_consumed() {
    support::session_context::group("settings_keep_raw_values_until_consumed");
}

#[test]
fn projection_reads_message_role_before_emission() {
    support::session_context::group("projection_reads_message_role_before_emission");
}

#[test]
fn index_construction_precedes_null_selection() {
    support::session_context::group("index_construction_precedes_null_selection");
}

#[test]
fn latest_compaction_retains_the_selected_entry() {
    support::session_context::group("latest_compaction_retains_the_selected_entry");
}

#[test]
fn selected_fields_keep_presence_until_their_consumer() {
    support::session_context::group("selected_fields_keep_presence_until_their_consumer");
}

#[test]
fn nested_duplicates_collapse_before_typed_views() {
    support::session_context::group("nested_duplicates_collapse_before_typed_views");
    let case = support::session_context::cases()
        .into_iter()
        .find(|case| case.id == "duplicates-at-selected-depth")
        .unwrap();
    let entries = maestro_session::parse_session_entries(case.args[0].as_str().unwrap());
    let context = maestro_session::build_session_context(
        &entries,
        Presence::Missing,
        None::<&std::collections::HashMap<_, _>>,
    )
    .unwrap();
    let typed = maestro_models::UserMessage::deserialize(&context.messages[0]).unwrap();
    assert_eq!(
        serde_json::to_value(typed).unwrap()["content"][0]["text"],
        "new"
    );
}

#[test]
fn escaped_identifiers_match_by_decoded_value() {
    support::session_context::group("escaped_identifiers_match_by_decoded_value");
}
#[test]
fn projected_context_reaches_the_shared_converter() {
    support::session_context::group("projected_context_reaches_the_shared_converter");
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn context_timestamps_use_local_time_only_for_offsetless_datetimes() {
    let name = "context_timestamps_use_local_time_only_for_offsetless_datetimes";
    if !maestro_test_process::selection().is_child() {
        if !maestro_test_process::local_zone_follows_tz() {
            return;
        }
        let output = maestro_test_process::TestProcess::current(name)
            .unwrap()
            .nocapture()
            .env("TZ", "America/New_York")
            .case(name)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );

        return;
    }
    support::session_context::group(name);
}

#[test]
fn typed_record_views_cover_each_entry_kind() {
    use serde_json::Value;
    let case = support::session_context::cases()
        .into_iter()
        .find(|case| case.test == "typed_record_views_cover_each_entry_kind")
        .unwrap();
    assert_eq!(case.operation, "parse");
    let entries = maestro_session::parse_session_entries(case.args[0].as_str().unwrap());
    support::session_context::assert_value(
        &Value::Array(entries.clone()),
        &case.expected["ok"],
        &case.id,
    );
    for entry in &entries {
        let view = typed_view(entry);
        assert_typed_fields(entry, &view);
    }
    assert_eq!(
        entries[0]["version"].as_u64(),
        Some(u64::from(maestro_session::CURRENT_SESSION_VERSION))
    );
    assert_optional_views();
    optional_entry_fields();
}

/// Check every declared field rather than accepting an empty typed view.
fn assert_typed_fields(entry: &serde_json::Value, view: &serde_json::Value) {
    let fields: &[&str] = match entry["type"].as_str().unwrap() {
        "session" => &["type", "id", "timestamp", "version", "cwd"],
        "message" => &["type", "id", "parentId", "timestamp", "message"],
        "thinking_level_change" => &["type", "id", "parentId", "timestamp", "thinkingLevel"],
        "model_change" => &["type", "id", "parentId", "timestamp", "provider", "modelId"],
        "compaction" => &[
            "type",
            "id",
            "parentId",
            "timestamp",
            "summary",
            "firstKeptEntryId",
            "tokensBefore",
            "details",
            "fromHook",
        ],
        "branch_summary" => &[
            "type",
            "id",
            "parentId",
            "timestamp",
            "summary",
            "fromId",
            "details",
            "fromHook",
        ],
        "custom" => &["type", "id", "parentId", "timestamp", "customType", "data"],
        "custom_message" => &[
            "type",
            "id",
            "parentId",
            "timestamp",
            "customType",
            "content",
            "details",
            "display",
        ],
        "label" => &["type", "id", "parentId", "timestamp", "targetId", "label"],
        "session_info" => &["type", "id", "parentId", "timestamp", "name"],
        _ => panic!("unexpected entry kind"),
    };
    assert_eq!(view.as_object().unwrap().len(), fields.len());
    for field in fields {
        support::session_context::assert_value(
            view.get(field).unwrap(),
            entry.get(field).unwrap(),
            field,
        );
    }
}

/// Exercise omitted, null and present optional view fields.
fn assert_optional_views() {
    use maestro_session::session_manager::*;
    use serde_json::json;
    for parent in [None, Some(serde_json::Value::Null), Some(json!("parent"))] {
        let mut value = json!({"type":"session", "id":"h", "timestamp":"time", "cwd":"work"});
        if let Some(parent) = parent {
            value["parentSession"] = parent;
        }
        let view = SessionHeader::deserialize(&value).unwrap();
        assert!(matches!(view.version, Presence::Missing));
        assert_eq!(serde_json::to_value(view).unwrap(), value);
    }
    let base = json!({"id":"a", "parentId": null, "timestamp":"time"});
    let fields = SessionEntryBase::deserialize(&base).unwrap();
    assert_eq!(fields.id, "a");
    assert_eq!(fields.parent_id, None);
    assert_eq!(fields.timestamp, "time");
    assert_eq!(serde_json::to_value(fields).unwrap(), base);
}

/// Decode an explicit view without copying its raw input.
fn typed_view(entry: &serde_json::Value) -> serde_json::Value {
    use maestro_session::session_manager::*;
    use serde_json::Value;
    match entry["type"].as_str().unwrap() {
        "session" => serde_json::to_value(SessionHeader::deserialize(entry).unwrap()).unwrap(),
        "message" => {
            serde_json::to_value(SessionMessageEntry::deserialize(entry).unwrap()).unwrap()
        }
        "thinking_level_change" => {
            serde_json::to_value(ThinkingLevelChangeEntry::deserialize(entry).unwrap()).unwrap()
        }
        "model_change" => {
            serde_json::to_value(ModelChangeEntry::deserialize(entry).unwrap()).unwrap()
        }
        "compaction" => {
            serde_json::to_value(CompactionEntry::<Value>::deserialize(entry).unwrap()).unwrap()
        }
        "branch_summary" => {
            serde_json::to_value(BranchSummaryEntry::<Value>::deserialize(entry).unwrap()).unwrap()
        }
        "custom" => {
            serde_json::to_value(CustomEntry::<Value>::deserialize(entry).unwrap()).unwrap()
        }
        "custom_message" => {
            serde_json::to_value(CustomMessageEntry::<Value>::deserialize(entry).unwrap()).unwrap()
        }
        "label" => serde_json::to_value(LabelEntry::deserialize(entry).unwrap()).unwrap(),
        "session_info" => {
            serde_json::to_value(SessionInfoEntry::deserialize(entry).unwrap()).unwrap()
        }
        _ => panic!("unexpected entry kind"),
    }
}

/// Verify optional fields retain their three states through ordinary views.
fn optional_entry_fields() {
    use serde_json::{Value, json};
    let case = support::session_context::cases()
        .into_iter()
        .find(|case| case.id == "entry-types")
        .unwrap();
    let entries = maestro_session::parse_session_entries(case.args[0].as_str().unwrap());
    for (kind, field, admitted) in [
        ("session", "version", json!(3)),
        ("session", "parentSession", json!("parent")),
        ("compaction", "details", json!({"z":1})),
        ("compaction", "fromHook", json!(true)),
        ("branch_summary", "details", json!({"b":2})),
        ("branch_summary", "fromHook", json!(true)),
        ("custom", "data", json!({"x":3})),
        ("custom_message", "details", json!([1, 2])),
        ("label", "label", json!("tag")),
        ("session_info", "name", json!("named")),
    ] {
        for state in [None, Some(Value::Null), Some(admitted.clone())] {
            let mut entry = entries
                .iter()
                .find(|value| value["type"] == kind)
                .unwrap()
                .clone();
            entry.as_object_mut().unwrap().shift_remove(field);
            if let Some(value) = &state {
                entry[field] = value.clone();
            }
            let view = typed_view(&entry);
            assert_eq!(
                view.get(field).is_some(),
                state.is_some(),
                "{kind}: {field}"
            );
            if let Some(state) = &state {
                support::session_context::assert_value(view.get(field).unwrap(), state, field);
            }
        }
    }
}

#[test]
fn distinct_entries_with_equal_ids_keep_finite_ancestry() {
    use serde_json::{Value, json};
    use std::collections::HashMap;
    let entries = [
        json!({"type":"message", "id":"same", "parentId":"root", "message":{"role":"user", "content":"leaf"}}),
    ];
    let root = json!({"type":"message", "id":"same", "parentId":null, "message":{"role":"user", "content":"root"}});
    let index = HashMap::from([("root".to_owned(), &root)]);
    for project in [
        maestro_session::build_session_context,
        maestro_session::session_manager::build_session_context,
    ] {
        let context = project(&entries, Presence::Missing, Some(&index)).unwrap();
        let contents: Vec<&Value> = context
            .messages
            .iter()
            .map(|message| &message["content"])
            .collect();
        assert_eq!(contents, [&json!("root"), &json!("leaf")]);
    }
}

#[test]
fn corpus_rejects_unowned_rows_and_duplicate_queries() {
    use support::session_context::{cases, validate_corpus};
    let mut rows = cases();
    rows[0].test = "unknown_owner".into();
    assert!(
        validate_corpus(&rows)
            .unwrap_err()
            .contains("unknown owner")
    );
    rows = cases();
    rows[1].id = rows[0].id.clone();
    assert!(validate_corpus(&rows).unwrap_err().contains("duplicate id"));
    rows = cases();
    rows[1].operation = rows[0].operation.clone();
    rows[1].args = rows[0].args.clone();
    assert!(
        validate_corpus(&rows)
            .unwrap_err()
            .contains("duplicate query")
    );
}
