//! Strict fixture replay of recorded session-tree operations through the public owner.
use chrono::DateTime;
use maestro_request::presence::Presence;
use maestro_session::SessionManager;
use maestro_session::session_manager::{ReadonlySessionManager, SessionTreeNode};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};
use std::{fs, io};

/// One recorded operation sequence with its complete observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Unique diagnostic label.
    name: String,
    /// Owning test function.
    test: String,
    /// Initial history, operations and reads.
    input: Input,
    /// Normalized operation results and observations.
    expected: Expected,
}
/// Initial history, mutations and reads of a case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Records loaded from a native file below a fixed header.
    entries: Option<Vec<Value>>,
    /// Complete native file text.
    jsonl: Option<String>,
    /// Mutations in order.
    #[serde(default)]
    ops: Vec<Op>,
    /// Reads in order, each a name followed by optional arguments; omitted means
    /// entries, selected leaf and tree.
    queries: Option<Vec<Vec<Value>>>,
}
/// One mutation with an optional name for its returned identity.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Op {
    /// Operation name.
    method: String,
    /// Arguments, absent ones omitted.
    args: Vec<Value>,
    /// Name later arguments use to refer to the returned identity.
    r#as: Option<String>,
}
/// Normalized results and observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// One result per operation.
    results: Vec<Value>,
    /// One observation per read.
    observations: Vec<Value>,
}

/// Fixed header written above loaded records.
const HEADER: &str = r#"{"type":"session","version":3,"id":"fixture-session","cwd":"/fixture","timestamp":"2025-01-01T00:00:00.000Z"}"#;

/// Replay every case owned by a test and compare complete normalized outcomes.
pub fn run(test: &str) {
    let corpus: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/session_tree.json")).unwrap();
    let source = format!(
        "{}\n{}",
        include_str!("../session_appends.rs"),
        include_str!("../session_tree.rs")
    );
    let mut names = HashSet::new();
    for case in &corpus {
        assert!(names.insert(&case.name), "duplicate case {}", case.name);
        assert!(
            source.contains(&format!("fn {}()", case.test)),
            "unowned case {}",
            case.name
        );
    }
    let selected: Vec<_> = corpus.iter().filter(|case| case.test == test).collect();
    assert!(!selected.is_empty(), "no cases for {test}");
    for case in selected {
        let actual = replay(case);
        assert!(
            same(&actual.0, &Value::Array(case.expected.results.clone())),
            "{} results: {}",
            case.name,
            actual.0
        );
        assert!(
            same(&actual.1, &Value::Array(case.expected.observations.clone())),
            "{} observations: {}",
            case.name,
            actual.1
        );
    }
}

/// Execute a case, returning normalized results and observations.
fn replay(case: &Case) -> (Value, Value) {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let directory = scratch.path().to_path_buf();
    let mut manager = open(&case.input, &directory);
    let mut named = HashMap::new();
    let mut generated = Vec::new();
    let mut results = Vec::new();
    for op in &case.input.ops {
        let outcome = apply(&mut manager, op, &named);
        if let Ok(Some(id)) = &outcome {
            if op.method != "branch" {
                generated.push(id.clone());
            }
            if let Some(name) = &op.r#as {
                named.insert(name.clone(), id.clone());
            }
        }
        results.push(match outcome {
            Ok(Some(id)) => json!({"ok": id}),
            Ok(None) => json!({"ok": null}),
            Err(error) => failure(&error),
        });
    }
    let default = ["getEntries", "getLeafId", "getTree"].map(|name| vec![json!(name)]);
    let observations: Vec<Value> = case
        .input
        .queries
        .as_deref()
        .unwrap_or(&default)
        .iter()
        .map(|query| observe(&manager, query, &named))
        .collect();
    assert_label_timestamps(&manager);
    let symbols = Normalizer::new(
        &manager,
        &generated,
        case.input.entries.is_none() && case.input.jsonl.is_none(),
    );
    let normalized = |values: Vec<Value>| symbols.visit(Value::Array(values), "");
    let outcome = (normalized(results), normalized(observations));
    drop(manager);
    let _ = fs::remove_dir_all(directory);
    outcome
}

/// Every resolved label timestamp equals the timestamp of the newest label entry for its node.
fn assert_label_timestamps(manager: &SessionManager) {
    fn walk(manager: &SessionManager, nodes: &[SessionTreeNode<'_>]) {
        for node in nodes {
            if let Some(stamp) = node.label_timestamp {
                let source = manager
                    .get_entries()
                    .into_iter()
                    .rev()
                    .find(|entry| entry["type"] == "label" && entry["targetId"] == node.entry["id"])
                    .expect("resolved label timestamp has a label entry");
                assert_eq!(&source["timestamp"], stamp, "label timestamp source");
            }
            walk(manager, &node.children);
        }
    }
    walk(manager, &manager.get_tree());
}

/// Memory history, or a native history loaded from the case's records.
fn open(input: &Input, directory: &std::path::Path) -> SessionManager {
    let text = match (&input.jsonl, &input.entries) {
        (Some(text), _) => text.clone(),
        (None, Some(entries)) => {
            let mut lines = vec![HEADER.to_owned()];
            lines.extend(entries.iter().map(Value::to_string));
            lines.join("\n")
        }
        (None, None) => return SessionManager::in_memory(Some("/fixture")).unwrap(),
    };
    fs::create_dir_all(directory).unwrap();
    let path = directory.join("history.jsonl");
    fs::write(&path, text).unwrap();
    SessionManager::open(path.to_str().unwrap(), None, None).unwrap()
}

/// Resolve a recorded argument, replacing a named reference with its identity.
fn resolve(arg: &Value, named: &HashMap<String, String>) -> Value {
    match arg.get("$ref").and_then(Value::as_str) {
        Some(name) => Value::String(named[name].clone()),
        None => arg.clone(),
    }
}
/// Text of a resolved argument.
fn text(args: &[Value], index: usize, named: &HashMap<String, String>) -> String {
    resolve(&args[index], named).as_str().unwrap().to_owned()
}
/// Omitted, null or present optional argument.
fn presence(args: &[Value], index: usize) -> Presence<Value> {
    match args.get(index) {
        None => Presence::Missing,
        Some(Value::Null) => Presence::Null,
        Some(value) => Presence::Present(value.clone()),
    }
}
/// Optional boolean argument.
fn flag(args: &[Value], index: usize) -> Presence<bool> {
    match presence(args, index) {
        Presence::Present(value) => Presence::Present(value.as_bool().unwrap()),
        Presence::Null => Presence::Null,
        Presence::Missing => Presence::Missing,
    }
}

/// Run one mutation; selection changes return no identity.
fn apply(
    manager: &mut SessionManager,
    op: &Op,
    named: &HashMap<String, String>,
) -> io::Result<Option<String>> {
    let args = &op.args;
    let id = match op.method.as_str() {
        "appendMessage" => manager.append_message(args[0].clone()),
        "appendThinkingLevelChange" => {
            manager.append_thinking_level_change(args[0].as_str().unwrap())
        }
        "appendModelChange" => {
            manager.append_model_change(args[0].as_str().unwrap(), args[1].as_str().unwrap())
        }
        "appendCompaction" => manager.append_compaction(
            args[0].as_str().unwrap(),
            args[1].as_str().unwrap(),
            args[2].as_f64().unwrap(),
            maestro_session::session_manager::CompactionOptions {
                details: presence(args, 3),
                from_hook: flag(args, 4),
                usage: maestro_session::Presence::Missing,
            },
        ),
        "appendCustomEntry" => {
            manager.append_custom_entry(args[0].as_str().unwrap(), presence(args, 1))
        }
        "appendSessionInfo" => manager.append_session_info(args[0].as_str().unwrap()),
        "appendCustomMessageEntry" => manager.append_custom_message_entry(
            args[0].as_str().unwrap(),
            serde_json::from_value(args[1].clone()).unwrap(),
            args[2].as_bool().unwrap(),
            presence(args, 3),
        ),
        "appendLabelChange" => {
            let label = presence(args, 1);
            manager.append_label_change(
                &text(args, 0, named),
                match &label {
                    Presence::Missing => Presence::Missing,
                    Presence::Null => Presence::Null,
                    Presence::Present(value) => Presence::Present(value.as_str().unwrap()),
                },
            )
        }
        "branchWithSummary" => manager.branch_with_summary(
            args[0].as_str(),
            args[1].as_str().unwrap(),
            maestro_session::session_manager::BranchSummaryOptions {
                details: presence(args, 2),
                from_hook: flag(args, 3),
                usage: maestro_session::Presence::Missing,
            },
        ),
        "branch" => return manager.branch(&text(args, 0, named)).map(|()| None),
        "resetLeaf" => {
            manager.reset_leaf();
            return Ok(None);
        }
        other => panic!("unknown method {other}"),
    };
    id.map(Some)
}

/// Observe one read as a complete outcome record.
fn observe(manager: &SessionManager, query: &[Value], named: &HashMap<String, String>) -> Value {
    let view: &dyn ReadonlySessionManager = manager;
    let target = || text(query, 1, named);
    let option = |value: Option<&Value>| json!({"ok": value});
    match query[0].as_str().unwrap() {
        "getCwd" => json!({"ok": view.get_cwd()}),
        "getSessionDir" => json!({"ok": view.get_session_dir()}),
        "getSessionId" => json!({"ok": view.get_session_id()}),
        "getSessionFile" => json!({"ok": view.get_session_file()}),
        "getLeafId" => json!({"ok": view.get_leaf_id()}),
        "getLeafEntry" => option(view.get_leaf_entry()),
        "getEntry" => option(view.get_entry(&target())),
        "getLabel" => option(view.get_label(&target())),
        "getBranch" => json!({"ok": view.get_branch(None).unwrap()}),
        "getHeader" => option(view.get_header()),
        "getEntries" => json!({"ok": view.get_entries()}),
        "getChildren" => json!({"ok": manager.get_children(&target())}),
        "getTree" => json!({"ok": view.get_tree().iter().map(tree_json).collect::<Vec<_>>()}),
        "getSessionName" => match view.get_session_name() {
            Ok(name) => json!({"ok": name}),
            Err(error) => failure(&error),
        },
        "buildSessionContext" => {
            json!({"ok": serde_json::to_value(manager.build_session_context().unwrap()).unwrap()})
        }
        other => panic!("unknown query {other}"),
    }
}

/// Recorded tree shape: entry, children, then any resolved label fields.
fn tree_json(node: &SessionTreeNode<'_>) -> Value {
    let mut object = Map::new();
    object.insert("entry".into(), node.entry.clone());
    object.insert(
        "children".into(),
        node.children.iter().map(tree_json).collect(),
    );
    if let Some(label) = node.label {
        object.insert("label".into(), label.clone());
    }
    if let Some(timestamp) = node.label_timestamp {
        object.insert("labelTimestamp".into(), timestamp.clone());
    }
    Value::Object(object)
}

/// Authored message for entry errors, the error category otherwise.
fn failure(error: &io::Error) -> Value {
    if error.kind() == io::ErrorKind::InvalidData {
        json!({"error": {"kind": "InvalidData"}})
    } else {
        json!({"error": {"message": error.to_string()}})
    }
}

/// Replaces generated identities, instants and message times by fixed symbols.
struct Normalizer {
    /// Generated identity to symbol.
    ids: HashMap<String, String>,
    /// Generated instants.
    times: HashSet<String>,
    /// Their epoch milliseconds, spelled as integers.
    millis: HashSet<String>,
}
impl Normalizer {
    /// Check every generated identity and instant has its public shape, then collect them.
    fn new(manager: &SessionManager, generated: &[String], generated_header: bool) -> Self {
        let header = manager.get_header().unwrap();
        let mut ids = HashMap::from([(
            header["id"].as_str().unwrap().to_owned(),
            "@session".to_owned(),
        )]);
        let mut times = HashSet::new();
        if generated_header {
            times.insert(header["timestamp"].as_str().unwrap().to_owned());
        }
        for (index, id) in generated.iter().enumerate() {
            assert!(
                id.len() == 36 || (id.len() == 8 && id.bytes().all(|b| b.is_ascii_hexdigit())),
                "identity shape"
            );
            ids.insert(id.clone(), format!("@entry-{index}"));
            let entry = manager.get_entry(id).unwrap();
            let time = entry["timestamp"].as_str().unwrap();
            assert!(
                time.len() == 24 && time.ends_with('Z') && time.as_bytes()[19] == b'.',
                "UTC millisecond shape"
            );
            times.insert(time.to_owned());
        }
        let millis = times
            .iter()
            .map(|time| {
                DateTime::parse_from_rfc3339(time)
                    .unwrap()
                    .timestamp_millis()
                    .to_string()
            })
            .collect();
        Self { ids, times, millis }
    }
    /// Replace identities everywhere and generated instants only under timestamp members.
    fn visit(&self, value: Value, key: &str) -> Value {
        let stamped = matches!(key, "timestamp" | "labelTimestamp");
        match value {
            Value::String(text) => {
                match (self.ids.get(&text), stamped && self.times.contains(&text)) {
                    (Some(symbol), _) => Value::String(symbol.clone()),
                    (None, true) => Value::String("@timestamp".into()),
                    (None, false) => Value::String(text),
                }
            }
            Value::Number(number) => match number.as_f64() {
                Some(millis) if key == "timestamp" && self.millis.contains(&millis.to_string()) => {
                    "@timestamp-millis".into()
                }
                _ => Value::Number(number),
            },
            Value::Array(items) => items
                .into_iter()
                .map(|item| self.visit(item, key))
                .collect(),
            Value::Object(object) => object
                .into_iter()
                .map(|(member, item)| {
                    let item = self.visit(item, &member);
                    (member, item)
                })
                .collect(),
            other => other,
        }
    }
}

/// Order-aware structural equality with one numeric domain.
pub fn same(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b)
                    .all(|((ka, a), (kb, b))| ka == kb && same(a, b))
        }
        _ => actual == expected,
    }
}
