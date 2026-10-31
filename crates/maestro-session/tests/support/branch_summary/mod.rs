//! Recorded branch scenarios.
use maestro_session::prepare_branch_entries;
#[cfg(not(target_arch = "wasm32"))]
use maestro_session::session_manager::ReadonlySessionManager;
#[cfg(not(target_arch = "wasm32"))]
use maestro_session::{SessionManager, collect_entries_for_branch_summary};
use serde::Deserialize;
use serde_json::Value;
#[cfg(not(target_arch = "wasm32"))]
use serde_json::json;
/// Complete consumed preparation recording.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Preparation {
    /// Operation cases.
    rows: Vec<Row>,
}
/// One recorded preparation operation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Diagnostic label.
    id: String,
    /// Owning test.
    test: String,
    /// Operation name.
    operation: String,
    /// Input operands.
    args: Value,
    /// Complete result.
    expected: Value,
    /// Consuming requests.
    calls: Vec<Value>,
}
/// Read the recorded preparation rows without silently accepting extra envelope fields.
fn preparation() -> Preparation {
    serde_json::from_str(include_str!("../../fixtures/branch_preparation.json")).unwrap()
}
/// Complete consumed collection, generation and completion recording.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Recording {
    /// Operation cases.
    rows: Vec<NativeRow>,
    /// Owned completion observation.
    lifetime: Lifetime,
}
/// One recorded collection or generation operation.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeRow {
    /// Diagnostic label.
    id: String,
    /// Owning test.
    test: String,
    /// Operation name.
    operation: String,
    /// Input operands.
    args: Value,
    /// Optional controlled response.
    response: Option<Value>,
    /// Complete result.
    expected: Value,
    /// Consuming requests.
    calls: Vec<Value>,
}
/// Read the recorded collection, generation and completion rows without silently accepting extra
/// envelope fields.
#[cfg(not(target_arch = "wasm32"))]
fn recording() -> Recording {
    serde_json::from_str(include_str!("../../fixtures/branch_summary.json")).unwrap()
}
/// Every preparation row of one public witness.
fn rows(name: &str) -> Vec<Row> {
    preparation()
        .rows
        .into_iter()
        .filter(|row| row.test == name)
        .collect()
}
/// Every collection, generation or completion row of one public witness, after the
/// canonical-input sign observation.
#[cfg(not(target_arch = "wasm32"))]
fn native_rows(name: &str) -> Vec<NativeRow> {
    let rows: Vec<_> = recording()
        .rows
        .into_iter()
        .filter(|row| row.test == name)
        .collect();
    for row in &rows {
        if row.id == "untrimmed-canonical-input" {
            let zero = row.args["entries"][1]["message"]["content"][1]["arguments"]["n"]
                .as_f64()
                .unwrap();
            assert!(zero == 0.0 && zero.is_sign_negative());
        }
    }
    rows
}

/// Prepare one recorded row through the pure preparation operation.
fn prepare(row: &Row) {
    let result = prepare_branch_entries(
        row.args["entries"].as_array().unwrap(),
        row.args.get("budget").and_then(Value::as_f64),
    );
    compare(
        result.map(|value| serde_json::to_value(value).unwrap()),
        row,
    );
    assert!(row.calls.is_empty());
}

/// Run every preparation row of one public witness.
pub fn prepare_group(name: &str) {
    let rows = rows(name);
    assert!(!rows.is_empty());
    for row in rows {
        assert_eq!(row.operation, "prepare");
        prepare(&row);
    }
}

/// Run every row belonging to one public witness.
#[cfg(not(target_arch = "wasm32"))]
pub fn group(name: &str) {
    let prepared = rows(name);
    let native = native_rows(name);
    assert!(!prepared.is_empty() || !native.is_empty());
    for row in prepared {
        assert_eq!(row.operation, "prepare");
        prepare(&row);
    }
    for row in native {
        match row.operation.as_str() {
            "collect" => collect(&row),
            "generate" => generation::run(&row),
            _ => panic!("unknown operation"),
        }
    }
}
/// Load real history and compare borrowed collection identities and retained state.
#[cfg(not(target_arch = "wasm32"))]
fn collect(row: &NativeRow) {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let path = super::native::below(scratch.path(), "history.jsonl");
    let mut lines = vec![json!({"type":"session","version":3,"id":"branch-fixture","cwd":"/fixture","timestamp":"2024-03-09T16:00:00.000Z"}).to_string()];
    lines.extend(
        row.args["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(Value::to_string),
    );
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    std::io::Write::write_all(&mut file, lines.join("\n").as_bytes()).unwrap();
    let session = SessionManager::open(&path, None, None).unwrap();
    let before = serde_json::to_value(session.get_entries()).unwrap();
    let leaf = session.get_leaf_id().map(str::to_owned);
    let result = collect_entries_for_branch_summary(
        &session,
        row.args["old"].as_str(),
        row.args["target"].as_str().unwrap(),
    )
    .unwrap();
    for entry in &result.entries {
        assert!(std::ptr::eq(
            *entry,
            session.get_entry(entry["id"].as_str().unwrap()).unwrap()
        ));
    }
    assert_eq!(row.expected["kind"], "value");
    super::value::assert_value(
        &serde_json::to_value(result).unwrap(),
        &row.expected["value"],
        &row.id,
    );
    assert_eq!(serde_json::to_value(session.get_entries()).unwrap(), before);
    assert_eq!(session.get_leaf_id(), leaf.as_deref());
    assert!(row.calls.is_empty());
    assert!(row.response.is_none());
}

/// Compare values or the native failure boundary.
fn compare<E: std::fmt::Debug>(actual: Result<Value, E>, row: &Row) {
    match row.expected["kind"].as_str().unwrap() {
        "value" => super::value::assert_value(&actual.unwrap(), &row.expected["value"], &row.id),
        "error" => assert!(actual.is_err(), "{}", row.id),
        _ => panic!("unknown outcome"),
    }
}

/// Controlled generation and retained completion.
#[cfg(not(target_arch = "wasm32"))]
mod generation;

/// Controlled ownership observations.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Lifetime {
    /// Public witness name.
    test: String,
    /// Ordered producer/caller observations.
    events: Vec<String>,
    /// Full generated result.
    result: Value,
}
/// Complete a pending producer after releasing all caller inputs.
#[cfg(not(target_arch = "wasm32"))]
pub fn lifetime() {
    generation::lifetime();
}
/// Send a request through a supplied stream function.
#[cfg(not(target_arch = "wasm32"))]
pub fn stream_function() {
    generation::stream_function();
}
