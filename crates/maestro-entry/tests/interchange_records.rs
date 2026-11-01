//! Session interchange records checked against recorded wire documents.
#![cfg(test)]
use maestro_entry::compaction::{CompactionPreparation, CompactionSettings};
use maestro_entry::session_manager::{
    BranchSummaryEntry, CompactionEntry, CustomEntry, CustomMessageEntry, LabelEntry,
    ModelChangeEntry, NavigateTreeOptions, NewSessionOptions, SessionContext, SessionContextModel,
    SessionHeader, SessionInfoEntry, SessionMessageEntry, ThinkingLevelChangeEntry,
};
use maestro_request::presence::Presence;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

/// A recorded wire document; both fields are consumed by the tests.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Case identity.
    id: String,
    /// Exact encoded document.
    text: String,
}

/// Reads the recorded documents without dropping unread fixture fields.
fn rows() -> Vec<(String, String)> {
    let rows: Vec<Row> =
        serde_json::from_str(include_str!("fixtures/interchange_records.json")).unwrap();
    rows.into_iter().map(|row| (row.id, row.text)).collect()
}

/// Checks values and the member sequence of every nested object.
fn same(actual: &Value, expected: &Value, id: &str) {
    match (actual, expected) {
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(
                a.keys().collect::<Vec<_>>(),
                b.keys().collect::<Vec<_>>(),
                "{id}"
            );
            for (k, v) in b {
                same(&a[k], v, id);
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{id}");
            a.iter().zip(b).for_each(|(a, b)| same(a, b, id));
        }
        (Value::Number(a), Value::Number(b)) => {
            assert_eq!(
                a.as_f64().unwrap().to_bits(),
                b.as_f64().unwrap().to_bits(),
                "{id}"
            );
        }
        _ => assert_eq!(actual, expected, "{id}"),
    }
}

/// Decodes a concrete record and compares its encoded members.
fn round<T: DeserializeOwned + Serialize>(text: &str, id: &str) {
    let record: T = serde_json::from_str(text).unwrap();
    let expected: Value = serde_json::from_str(text).unwrap();
    same(&serde_json::to_value(&record).unwrap(), &expected, id);
}

#[test]
fn session_operation_options_decode_without_losing_presence() {
    for (text, parent) in [
        ("{}", Presence::Missing),
        (r#"{"parentSession":null}"#, Presence::Null),
        (
            r#"{"parentSession":"  parent  "}"#,
            Presence::Present("  parent  ".into()),
        ),
    ] {
        let options: NewSessionOptions<Presence<String>> = serde_json::from_str(text).unwrap();
        assert_eq!(options.id, None);
        assert_eq!(options.parent_session, parent);
    }
    let guest: NewSessionOptions<Option<String>> = serde_json::from_str("{}").unwrap();
    assert_eq!(guest.parent_session, None);
    let options: NavigateTreeOptions = serde_json::from_str(r#"{"summarize":false,"customInstructions":" focus ","replaceInstructions":true,"label":""}"#).unwrap();
    assert_eq!(options.summarize, Some(false));
    assert_eq!(options.custom_instructions.as_deref(), Some(" focus "));
    assert_eq!(options.replace_instructions, Some(true));
    assert_eq!(options.label.as_deref(), Some(""));
}

#[test]
fn entry_records_keep_session_file_members() {
    let mut n = 0;
    for (id, text) in rows() {
        let document: Value = serde_json::from_str(&text).unwrap();
        let kind = &document["type"];
        match kind.as_str() {
            Some("session") => round::<SessionHeader>(&text, &id),
            Some("message") => round::<SessionMessageEntry<Value>>(&text, &id),
            Some("thinking_level_change") => round::<ThinkingLevelChangeEntry>(&text, &id),
            Some("model_change") => round::<ModelChangeEntry>(&text, &id),
            Some("compaction") => round::<CompactionEntry>(&text, &id),
            Some("branch_summary") => round::<BranchSummaryEntry>(&text, &id),
            Some("custom") => round::<CustomEntry>(&text, &id),
            Some("custom_message") => round::<CustomMessageEntry>(&text, &id),
            Some("label") => round::<LabelEntry>(&text, &id),
            Some("session_info") => round::<SessionInfoEntry>(&text, &id),
            _ => continue,
        }
        n += 1;
    }
    assert_eq!(n, 16);
}

#[test]
fn context_views_omit_only_missing_values() {
    let rows = rows();
    for (id, context) in native_contexts() {
        let text = &rows.iter().find(|r| r.0 == id).unwrap().1;
        assert_eq!(&serde_json::to_string(&context).unwrap(), text, "{id}");
    }
    let guest = SessionContext::<Value, String> {
        messages: vec![],
        thinking_level: "high".into(),
        model: Some(SessionContextModel {
            provider: "p".into(),
            model_id: "m".into(),
        }),
    };
    let text = &rows
        .iter()
        .find(|r| r.0 == "context-model-change")
        .unwrap()
        .1;
    assert_eq!(&serde_json::to_string(&guest).unwrap(), text);
}

#[test]
fn compaction_records_keep_reference_members() {
    let mut count = 0;
    for (id, text) in rows() {
        if id.starts_with("preparation") {
            round::<CompactionPreparation<Value>>(&text, &id);
            let mut record: CompactionPreparation<Value> = serde_json::from_str(&text).unwrap();
            assert!(!record.file_ops.read.insert("z".into()));
            assert_eq!(
                record
                    .file_ops
                    .read
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                ["z", "a"]
            );
            count += 1;
        } else if id == "settings-default" {
            round::<CompactionSettings>(&text, &id);
            count += 1;
        }
    }
    assert_eq!(count, 3);
}

/// Details can be selected without supplying a default value for their type.
#[test]
fn detail_records_decode_types_without_default() {
    #[derive(Debug, serde::Deserialize, PartialEq)]
    #[serde(deny_unknown_fields)]
    struct Detail {
        /// Selected payload.
        value: String,
    }
    for kind in ["compaction", "branch_summary", "custom_message", "custom"] {
        let mut value = json!({"type":kind,"id":"selected","parentId":null,"timestamp":"literal",
            "summary":"recap","firstKeptEntryId":"kept","tokensBefore":1.5,"fromId":"root",
            "customType":"state","content":"hello","display":true});
        let field = if kind == "custom" { "data" } else { "details" };
        for present in [false, true] {
            if present {
                value[field] = json!({"value":"selected"});
            }
            let details = match kind {
                "compaction" => {
                    serde_json::from_value::<CompactionEntry<Detail>>(value.clone())
                        .unwrap()
                        .details
                }
                "branch_summary" => {
                    serde_json::from_value::<BranchSummaryEntry<Detail>>(value.clone())
                        .unwrap()
                        .details
                }
                "custom_message" => {
                    serde_json::from_value::<CustomMessageEntry<Detail>>(value.clone())
                        .unwrap()
                        .details
                }
                "custom" => {
                    serde_json::from_value::<CustomEntry<Detail>>(value.clone())
                        .unwrap()
                        .data
                }
                _ => unreachable!(),
            };
            assert_eq!(
                details,
                if present {
                    Presence::Present(Detail {
                        value: "selected".into(),
                    })
                } else {
                    Presence::Missing
                }
            );
        }
    }
}

/// Constructs the native model view without defaulting either observation.
fn context_model(
    provider: Presence<Value>,
    model_id: Presence<Value>,
) -> SessionContextModel<Presence<Value>> {
    SessionContextModel { provider, model_id }
}

/// Independently constructed contexts exercise each selected optional state.
fn native_contexts() -> [(&'static str, SessionContext<Value, Presence<Value>>); 7] {
    type Native = SessionContext<Value, Presence<Value>>;
    let off = || Presence::Present(json!("off"));
    let context = |level, model| Native {
        messages: vec![],
        thinking_level: level,
        model,
    };
    let mut assistant = context(
        off(),
        Some(context_model(
            Presence::Present(json!("p")),
            Presence::Present(json!("m")),
        )),
    );
    assistant
        .messages
        .push(json!({"role":"assistant","provider":"p","model":"m","content":[],"timestamp":1.5}));
    [
        ("context-default", context(off(), None)),
        ("context-thinking-missing", context(Presence::Missing, None)),
        ("context-thinking-null", context(Presence::Null, None)),
        (
            "context-model-change",
            context(
                Presence::Present(json!("high")),
                Some(context_model(
                    Presence::Present(json!("p")),
                    Presence::Present(json!("m")),
                )),
            ),
        ),
        (
            "context-model-missing-provider",
            context(
                off(),
                Some(context_model(
                    Presence::Missing,
                    Presence::Present(json!("m")),
                )),
            ),
        ),
        (
            "context-model-null-provider",
            context(
                off(),
                Some(context_model(Presence::Null, Presence::Missing)),
            ),
        ),
        ("context-assistant-model", assistant),
    ]
}

#[test]
fn compaction_result_keeps_optional_details_and_member_order() {
    use maestro_entry::compaction::CompactionResult;
    #[derive(Debug, serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Detail {
        /// Selected payload.
        value: String,
    }
    for (id, text) in rows() {
        if id.starts_with("result-") {
            round::<CompactionResult>(&text, &id);
            let result: CompactionResult = serde_json::from_str(&text).unwrap();
            assert_eq!(serde_json::to_string(&result).unwrap(), text, "{id}");
            if id == "result-details-string" {
                round::<CompactionResult<String>>(&text, &id);
            }
        }
    }
    for details in [None, Some(json!({"value":"selected"}))] {
        let present = details.is_some();
        let mut value = json!({"summary":"summary","firstKeptEntryId":"kept","tokensBefore":1});
        if let Some(details) = details {
            value["details"] = details;
        }
        let result: CompactionResult<Detail> = serde_json::from_value(value).unwrap();
        match (present, result.details) {
            (false, Presence::Missing) => (),
            (true, Presence::Present(detail)) => assert_eq!(detail.value, "selected"),
            _ => panic!("detail presence differs"),
        }
    }
}
