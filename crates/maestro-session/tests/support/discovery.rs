//! Recorded listing cases written to disposable session files and compared field by field.
use maestro_request::presence::Presence;
use maestro_session::SessionInfo;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::Path;

/// One recorded history and the metadata its listing must return.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    input: String,
    mtime: u64,
    expected: Option<Value>,
}

/// Render a presence in the recorded tag form: `{"absent": true}`, `null` or `{"present": value}`.
fn tag(value: &Presence<Value>) -> Value {
    match value {
        Presence::Missing => json!({"absent": true}),
        Presence::Null => Value::Null,
        Presence::Present(value) => json!({"present": value}),
    }
}

/// Compare an epoch value, where `{"invalid": true}` is `NaN`.
fn assert_date(actual: f64, expected: &Value, label: &str) {
    if expected["invalid"] == true {
        assert!(actual.is_nan(), "{label}");
    } else {
        assert_eq!(
            actual.to_bits(),
            expected.as_f64().unwrap().to_bits(),
            "{label}"
        );
    }
}

/// Compare every returned member with its recorded value.
fn assert_info(actual: &SessionInfo, path: &str, expected: &Value, label: &str) {
    assert_eq!(actual.path, path, "{label}");
    assert_eq!(tag(&actual.id), expected["id"], "{label}");
    assert_eq!(actual.cwd, expected["cwd"].as_str().unwrap(), "{label}");
    match &expected["name"] {
        Value::String(name) => assert_eq!(actual.name.as_deref(), Some(name.as_str()), "{label}"),
        absent => {
            assert_eq!(absent["absent"], true, "{label}");
            assert_eq!(actual.name, None, "{label}");
        }
    }
    assert_eq!(
        tag(&actual.parent_session_path),
        expected["parentSessionPath"],
        "{label}"
    );
    assert_date(actual.created, &expected["created"], label);
    assert_date(actual.modified, &expected["modified"], label);
    assert_eq!(
        actual.message_count as u64,
        expected["messageCount"].as_u64().unwrap(),
        "{label}"
    );
    assert_eq!(
        actual.first_message,
        expected["firstMessage"].as_str().unwrap(),
        "{label}"
    );
    assert_eq!(
        actual.all_messages_text,
        expected["allMessagesText"].as_str().unwrap(),
        "{label}"
    );
}

/// List each named recorded case in its own directory and compare the whole result.
/// `adjust` may replace an expected value that depends on the process timezone.
pub fn replay(names: &[&str], adjust: impl Fn(&str, &mut Value)) {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/session_discovery.json")).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    for name in names {
        let case = cases.iter().find(|case| case.name == *name).unwrap();
        let scratch = super::scratch::Scratch::new();
        let path = Path::new(scratch.text()).join("case.jsonl");
        std::fs::write(&path, &case.input).unwrap();
        let file = std::fs::File::options().write(true).open(&path).unwrap();
        file.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_millis(case.mtime))
            .unwrap();
        let listed = runtime
            .block_on(maestro_session::SessionManager::list(
                "/unused",
                Some(scratch.text()),
                "/unused",
                None,
            ))
            .unwrap();
        let path = path.to_str().unwrap();
        match &case.expected {
            None => assert!(listed.is_empty(), "{name}"),
            Some(expected) => {
                let mut expected = expected.clone();
                adjust(name, &mut expected);
                assert_eq!(listed.len(), 1, "{name}");
                assert_info(&listed[0], path, &expected, name);
            }
        }
    }
}
