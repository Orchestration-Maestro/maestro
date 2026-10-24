//! Recorded context calls.
use maestro_request::presence::Presence;
use maestro_session::session_manager::*;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

/// One recorded call.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Diagnostic label.
    pub id: String,
    /// Owning witness.
    pub test: String,
    /// Invoked operation.
    pub operation: String,
    /// Supplied arguments.
    pub args: Vec<Value>,
    /// Full observed output.
    pub expected: Value,
}
/// Read the strict corpus.
pub fn cases() -> Vec<Case> {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/session_context.json")).unwrap();
    validate_corpus(&cases).unwrap();
    cases
}
/// Reject unowned or duplicate recorded queries.
pub fn validate_corpus(cases: &[Case]) -> Result<(), String> {
    let source: Vec<_> = include_str!("../context_projection.rs").lines().collect();
    let owners: std::collections::HashSet<_> = source
        .windows(2)
        .filter(|lines| lines[0] == "#[test]")
        .filter_map(|lines| {
            lines[1]
                .strip_prefix("fn ")?
                .split_once('(')
                .map(|(name, _)| name)
        })
        .collect();
    let mut ids = std::collections::HashSet::new();
    let mut queries = std::collections::HashSet::new();
    for case in cases {
        if !owners.contains(case.test.as_str()) {
            return Err(format!("{}: unknown owner {}", case.id, case.test));
        }
        if !ids.insert(&case.id) {
            return Err(format!("{}: duplicate id", case.id));
        }
        let query = serde_json::to_string(&(&case.operation, &case.args)).unwrap();
        if !queries.insert(query) {
            return Err(format!("{}: duplicate query", case.id));
        }
    }
    Ok(())
}
/// Remove undefined metadata at property boundaries and decode signed zero.
pub fn decode(value: &Value) -> Value {
    match value {
        Value::Object(object) if object.get("$number").and_then(Value::as_str) == Some("-0") => {
            serde_json::json!(-0.0)
        }
        Value::Object(object) => Value::Object(
            object
                .iter()
                .filter(|(_, v)| !undefined(v))
                .map(|(k, v)| (k.clone(), decode(v)))
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(decode).collect()),
        value => value.clone(),
    }
}
/// Whether a property was absent in the recording.
pub fn undefined(value: &Value) -> bool {
    value.get("$undefined") == Some(&Value::Bool(true))
}
/// Project recorded supplied entries and optional index.
pub fn context(args: &[Value]) -> Result<SessionContext, serde_json::Error> {
    let entries = decode(&args[0]);
    let entries = entries.as_array().unwrap();
    let leaf = match args.get(1) {
        None => Presence::Missing,
        Some(value) if undefined(value) => Presence::Missing,
        Some(Value::Null) => Presence::Null,
        Some(value) => Presence::Present(value.as_str().unwrap()),
    };
    let supplied = args.get(2).filter(|value| !undefined(value)).map(decode);
    let index: Option<HashMap<String, &Value>> = supplied.as_ref().map(|value| {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|pair| {
                let pair = pair.as_array().unwrap();
                (pair[0].as_str().unwrap().to_owned(), &pair[1])
            })
            .collect()
    });
    build_session_context(entries, leaf, index.as_ref())
}
/// Assert a recorded context group.
pub fn group(name: &str) {
    let selected: Vec<_> = cases()
        .into_iter()
        .filter(|case| case.test == name)
        .collect();
    assert!(!selected.is_empty());
    for case in selected {
        let result = match case.operation.as_str() {
            "context_model" => context(&case.args).map(|context| serde_json::json!({"modelMessages": model_messages(&context), "context": context})),
            "context" | "cycle" => context(&case.args).and_then(serde_json::to_value),
            "latest" | "latest_identity" => latest(&case),
            "parse_context" => {
                let entries = parse_session_entries(case.args[0].as_str().unwrap());
                build_session_context(&entries, Presence::Missing, None::<&std::collections::HashMap<_, _>>).and_then(serde_json::to_value)
            }
            "keys" => {
                let entries = parse_session_entries(case.args[0].as_str().unwrap());
                let value = &entries[0];
                Ok(serde_json::json!({"keys": value.as_object().unwrap().keys().collect::<Vec<_>>(), "value": value, "hasData": value.get("data").is_some(), "hasDetails": value.get("details").is_some()}))
            }
            "parse" => Ok(Value::Array(parse_session_entries(case.args[0].as_str().unwrap()))),
            _ => panic!("{}: unsupported operation", case.id),
        };
        assert_result(result, &case);
    }
}

pub use super::value::assert_equal;

/// Assert complete success or the reached native failure field.
pub fn assert_result(result: Result<Value, serde_json::Error>, case: &Case) {
    if let Some(error) = case.expected.get("error") {
        assert_eq!(error.as_object().unwrap().len(), 1);
        let field = error["field"].as_str().unwrap();
        assert!(
            result.unwrap_err().to_string().contains(field),
            "{}: {field}",
            case.id
        );
    } else {
        assert_eq!(case.expected.as_object().unwrap().len(), 1);
        assert_equal(&result.unwrap(), &decode(&case.expected["ok"]), &case.id);
    }
}

/// Convert the projected raw values through the shared message owner.
pub fn model_messages(context: &SessionContext) -> Value {
    use maestro_agent::AgentMessage;
    use maestro_models::{Message, UserMessage};
    use maestro_session::message_records::{
        BranchSummaryMessage, CompactionSummaryMessage, CustomAgentMessages, CustomMessage,
    };
    use std::sync::{Arc, RwLock};
    let messages: Vec<AgentMessage<CustomAgentMessages>> = context
        .messages
        .iter()
        .map(|value| match value["role"].as_str().unwrap() {
            "user" => AgentMessage::User(Arc::new(RwLock::new(
                UserMessage::deserialize(value).unwrap(),
            ))),
            "custom" => AgentMessage::Custom(Arc::new(RwLock::new(CustomAgentMessages::Custom(
                CustomMessage::deserialize(value).unwrap(),
            )))),
            "branchSummary" => {
                AgentMessage::Custom(Arc::new(RwLock::new(CustomAgentMessages::BranchSummary(
                    BranchSummaryMessage::deserialize(value).unwrap(),
                ))))
            }
            "compactionSummary" => AgentMessage::Custom(Arc::new(RwLock::new(
                CustomAgentMessages::CompactionSummary(
                    CompactionSummaryMessage::deserialize(value).unwrap(),
                ),
            ))),
            _ => panic!("unexpected role"),
        })
        .collect();
    let projected: Vec<Message> = maestro_session::convert_to_llm(&messages);
    serde_json::to_value(projected).unwrap()
}

/// Observe reverse lookup values or original-element identity.
fn latest(case: &Case) -> Result<Value, serde_json::Error> {
    let entries = decode(&case.args[0]);
    let entries = entries.as_array().unwrap();
    let entry = get_latest_compaction_entry(entries)?;
    if case.operation == "latest_identity" {
        let index = entries
            .iter()
            .position(|candidate| std::ptr::eq(candidate, entry.unwrap()))
            .unwrap();
        return Ok(serde_json::json!({"selectedIndex": index}));
    }
    Ok(entry.cloned().unwrap_or(Value::Null))
}
