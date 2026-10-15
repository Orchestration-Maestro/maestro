//! Recorded branch scenarios.
use maestro_session::session_manager::ReadonlySessionManager;
use maestro_session::{SessionManager, collect_entries_for_branch_summary, prepare_branch_entries};
use serde::Deserialize;
use serde_json::{Value, json};
/// Complete consumed fixture.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Operation cases.
    rows: Vec<Row>,
    /// Owned completion observation.
    lifetime: Lifetime,
}
/// One recorded operation.
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
    /// Optional controlled response.
    response: Option<Value>,
    /// Complete result.
    expected: Value,
    /// Consuming requests.
    calls: Vec<Value>,
}
/// Read the recorded corpus without silently accepting extra envelope fields.
fn corpus() -> Corpus {
    serde_json::from_str(include_str!("../../fixtures/branch_summary.json")).unwrap()
}
/// Run every row belonging to one public witness.
pub fn group(name: &str) {
    let rows: Vec<_> = corpus()
        .rows
        .into_iter()
        .filter(|row| row.test == name)
        .collect();
    assert!(!rows.is_empty());
    for row in rows {
        if row.id == "untrimmed-canonical-input" {
            let zero = row.args["entries"][1]["message"]["content"][1]["arguments"]["n"]
                .as_f64()
                .unwrap();
            assert!(zero == 0.0 && zero.is_sign_negative());
        }
        match row.operation.as_str() {
            "collect" => collect(&row),
            "prepare" => {
                let result = prepare_branch_entries(
                    row.args["entries"].as_array().unwrap(),
                    row.args.get("budget").and_then(Value::as_f64),
                );
                compare(
                    result.map(|value| serde_json::to_value(value).unwrap()),
                    &row,
                );
                assert!(row.calls.is_empty());
            }
            "generate" => generation::run(&row),
            _ => panic!("unknown operation"),
        }
    }
}
/// Load real history and compare borrowed collection identities and retained state.
fn collect(row: &Row) {
    let scratch = super::scratch::Scratch::new();
    let path = format!("{}/history.jsonl", scratch.text());
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
mod generation;

/// Controlled ownership observations.
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
pub fn lifetime() {
    generation::lifetime();
}
