//! Recorded public memory-history operations.
use chrono::{DateTime, Utc};
use maestro_request::presence::Presence;
use maestro_session::session_manager::ReadonlySessionManager;
use maestro_session::{NewSessionOptions, SessionManager};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

/// Public operation sequence and complete observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Unique diagnostic label.
    pub id: String,
    /// Owning test name.
    pub test: String,
    /// Authored directory or an absence marker.
    pub cwd: Value,
    /// Ordered calls.
    pub steps: Vec<Step>,
    /// Complete ordered observations.
    pub expected: Value,
}
/// A public call with only its consumed arguments.
#[derive(Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "lowercase", deny_unknown_fields)]
pub enum Step {
    /// Append an open message.
    Append { message: Value },
    /// Select an entry.
    Branch { target: Value },
    /// Read an ancestry.
    Path { target: Value },
    /// Read an indexed entry.
    Entry { target: Value },
    /// Replace the session.
    New { options: Value },
}
/// Captured generated identities and time bounds, not payload transformations.
struct Replay {
    /// Generated identities mapped to their corpus symbols.
    sessions: HashMap<String, String>,
    /// All returned entry identities, including entries removed by reset.
    entries: Vec<String>,
    /// Earliest instant of this operation sequence.
    start: DateTime<Utc>,
    /// Whether the directory was omitted.
    default_cwd: bool,
}
/// Encode absence without flattening it into JSON null.
fn absent() -> Value {
    json!({"$undefined": true})
}
/// Exact source absence marker.
fn undefined(value: &Value) -> bool {
    value == &absent()
}
/// Returned member sequence, independent of map equality.
pub fn keys(value: &Value) -> Vec<&str> {
    value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect()
}
impl Replay {
    /// Validate and capture a newly generated `UUIDv7`.
    fn session(&mut self, manager: &SessionManager) {
        let id = manager.get_session_id();
        let uuid = uuid::Uuid::parse_str(id).unwrap();
        assert_eq!(uuid.get_version_num(), 7);
        assert_eq!(uuid.to_string(), id);
        assert!(!self.sessions.contains_key(id));
        assert!(self.sessions.keys().all(|previous| previous.as_str() < id));
        let symbol = format!("$session{}", self.sessions.len());
        self.sessions.insert(id.to_owned(), symbol);
    }
    /// Validate UTC emission before replacing a generated timestamp.
    fn timestamp(&self, value: &Value) {
        let text = value.as_str().unwrap();
        assert_eq!(text.len(), 24);
        assert!(text.ends_with('Z'));
        let parsed = DateTime::parse_from_rfc3339(text)
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(
            parsed.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            text
        );
        assert!(parsed.timestamp_millis() >= self.start.timestamp_millis());
        assert!(parsed <= Utc::now());
    }
    /// Replace only a captured generated identity.
    fn identity(&self, value: &Value) -> Value {
        let Some(id) = value.as_str() else {
            return value.clone();
        };
        if let Some(symbol) = self.sessions.get(id) {
            return symbol.clone().into();
        }
        self.entries
            .iter()
            .position(|entry| entry == id)
            .map_or_else(
                || value.clone(),
                |position| format!("$entry{position}").into(),
            )
    }
    /// Normalize generated entry metadata; the raw message subtree is untouched.
    fn entry(&self, entry: &Value) -> Value {
        let mut value = entry.clone();
        value["id"] = self.identity(&entry["id"]);
        value["parentId"] = self.identity(&entry["parentId"]);
        self.timestamp(&entry["timestamp"]);
        value["timestamp"] = "$timestamp".into();
        value
    }
    /// Normalize a current header, retaining its original member order.
    fn header(&self, manager: &SessionManager) -> Value {
        let mut header = manager.get_header().unwrap().clone();
        header["id"] = self.identity(&header["id"]);
        self.timestamp(&header["timestamp"]);
        header["timestamp"] = "$timestamp".into();
        if self.default_cwd {
            assert_eq!(
                manager.get_cwd(),
                std::env::current_dir().unwrap().to_string_lossy()
            );
            header["cwd"] = "$cwd".into();
        }
        header
    }
    /// Observe every public read over the same owner.
    fn snapshot(&self, manager: &SessionManager) -> Value {
        let entries = manager.get_entries();
        json!({
            "persisted": manager.is_persisted(),
            "cwd": if self.default_cwd { "$cwd" } else { manager.get_cwd() },
            "sessionDir": manager.get_session_dir(),
            "sessionId": self.identity(&Value::from(manager.get_session_id())),
            "sessionFile": manager.get_session_file().map_or_else(absent, Value::from),
            "header": self.header(manager), "headerKeys": keys(manager.get_header().unwrap()),
            "entries": entries.iter().map(|entry| self.entry(entry)).collect::<Vec<_>>(),
            "entryKeys": entries.iter().map(|entry| keys(entry)).collect::<Vec<_>>(),
            "messageKeys": entries.iter().map(|entry| keys(&entry["message"])).collect::<Vec<_>>(),
            "leafId": self.identity(&json!(manager.get_leaf_id())),
            "leafEntry": manager.get_leaf_entry().map_or_else(absent, |entry| self.entry(entry)),
            "branch": manager.get_branch(None).unwrap().iter().map(|entry| self.entry(entry)).collect::<Vec<_>>(),
            "context": {"ok": manager.build_session_context().unwrap()},
        })
    }
    /// Resolve an index operand against all prior returned ids, including removed ids.
    fn target(&self, target: &Value) -> Option<String> {
        if undefined(target) {
            return None;
        }
        Some(target.as_u64().map_or_else(
            || target.as_str().unwrap().to_owned(),
            |index| self.entries[usize::try_from(index).unwrap()].clone(),
        ))
    }
    /// Drive one call and encode its actual return or failure.
    fn step(&mut self, manager: &mut SessionManager, step: &Step) -> (&'static str, Value) {
        match step {
            Step::Append { message } => {
                let id = manager.append_message(message.clone()).unwrap();
                assert_eq!(id.len(), 8);
                assert!(
                    id.bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                );
                assert!(!self.entries.contains(&id));
                self.entries.push(id.clone());
                ("append", json!({"ok": self.identity(&id.into())}))
            }
            Step::New { options } => {
                let options = new_options(options);
                let generated = options.id.is_none();
                let result = manager.new_session(options);
                assert!(result.is_none());
                if generated {
                    self.session(manager);
                }
                ("new", json!({"ok": absent()}))
            }
            Step::Branch { target } => {
                let id = self.target(target).unwrap();
                let result = match manager.branch(&id) {
                    Ok(()) => json!({"ok": absent()}),
                    Err(error) => {
                        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
                        assert_eq!(error.to_string(), format!("Entry {id} not found"));
                        json!({"error": {"message": format!("Entry {} not found", self.identity(&id.into()).as_str().unwrap())}})
                    }
                };
                ("branch", result)
            }
            Step::Path { target } => {
                let id = self.target(target);
                let path = manager.get_branch(id.as_deref()).unwrap();
                (
                    "path",
                    json!({"ok": path.iter().map(|entry| self.entry(entry)).collect::<Vec<_>>()}),
                )
            }
            Step::Entry { target } => {
                let id = self.target(target).unwrap();
                let entry = manager
                    .get_entry(&id)
                    .map_or_else(absent, |entry| self.entry(entry));
                ("entry", json!({"ok": entry}))
            }
        }
    }
}
/// Consume only declared option fields, distinguishing omitted/null/empty.
fn new_options(value: &Value) -> NewSessionOptions {
    if undefined(value) {
        return NewSessionOptions::default();
    }
    assert!(
        keys(value)
            .iter()
            .all(|key| matches!(*key, "id" | "parentSession"))
    );
    NewSessionOptions {
        id: value
            .get("id")
            .filter(|value| !value.is_null())
            .map(|value| value.as_str().unwrap().to_owned()),
        parent_session: match value.get("parentSession") {
            None => Presence::Missing,
            Some(Value::Null) => Presence::Null,
            Some(value) => Presence::Present(value.as_str().unwrap().to_owned()),
        },
    }
}
/// Strictly decode the public sequence subset from the shared fixture.
fn cases() -> Vec<Case> {
    let rows: Vec<Value> =
        serde_json::from_str(include_str!("../fixtures/session_history.json")).unwrap();
    assert_eq!(rows.len(), 37);
    let mut ids = HashSet::new();
    for row in &rows {
        assert!(ids.insert(row["id"].as_str().unwrap()));
        let owner = format!("fn {}()", row["test"].as_str().unwrap());
        assert!(
            include_str!("../session_history.rs").contains(&owner)
                || include_str!("../../src/session_manager/tests.rs").contains(&owner)
        );
        assert!(
            row.get("steps").is_some()
                || matches!(
                    row["operation"].as_str(),
                    Some("generate_id" | "native_cwd")
                )
        );
    }
    rows.into_iter()
        .filter(|row| row.get("steps").is_some())
        .map(|row| serde_json::from_value(row).unwrap())
        .collect()
}
/// Run exactly the cases owned by the named test.
pub fn group(name: &str) {
    let cases = cases();
    let mut ids = HashSet::new();
    let mut queries = HashSet::new();
    for case in &cases {
        assert!(ids.insert(&case.id));
        assert!(queries.insert(serde_json::to_string(&json!([case.cwd, case.steps])).unwrap()));
        assert!(include_str!("../session_history.rs").contains(&format!("fn {}()", case.test)));
    }
    let selected: Vec<_> = cases.into_iter().filter(|case| case.test == name).collect();
    assert!(!selected.is_empty());
    for case in selected {
        replay(&case);
    }
}
/// Compare the complete ordered transcript, including nested member order.
fn replay(case: &Case) {
    let start = Utc::now();
    let default_cwd = undefined(&case.cwd);
    let cwd = if default_cwd {
        None
    } else {
        Some(case.cwd.as_str().unwrap())
    };
    let mut manager = SessionManager::in_memory(cwd).unwrap();
    let mut replay = Replay {
        sessions: HashMap::new(),
        entries: Vec::new(),
        start,
        default_cwd,
    };
    replay.session(&manager);
    let mut observations = vec![json!({"op":"initial", "state": replay.snapshot(&manager)})];
    for step in &case.steps {
        let (op, result) = replay.step(&mut manager, step);
        observations.push(json!({"op": op, "result": result, "state": replay.snapshot(&manager)}));
    }
    assert_eq!(
        serde_json::to_string(&observations).unwrap(),
        serde_json::to_string(&case.expected).unwrap(),
        "case {}",
        case.id
    );
}
