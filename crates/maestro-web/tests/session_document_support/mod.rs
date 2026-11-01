//! Strict corpus inputs and public observations.
#![cfg(test)]
use base64::Engine as _;
use maestro_web::export_html::template::{SessionDocument, load_document};
use serde::Deserialize;
use serde_json::{Value, json};

/// Committed oracle cases.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    /// Ordered cases.
    pub rows: Vec<Row>,
}
/// One recorded input and its observed outcome.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    /// Unique input label.
    pub name: String,
    /// Loader or accounting operands.
    pub input: Input,
    /// Success or error.
    pub branch: String,
    /// Successful public observations.
    pub output: Option<Value>,
    /// Native failure category.
    #[serde(rename = "errorKind")]
    pub error_kind: Option<String>,
}
/// Raw input framing without a second document schema.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    /// Complete exported object.
    pub data: Option<Value>,
    /// Authored JSON, retaining duplicate members and signed zero.
    pub raw: Option<String>,
    /// Authored base64 bytes.
    pub base64: Option<String>,
    /// Browser query text.
    pub query: Option<String>,
    /// Injected query text, including present-empty.
    pub injected: Option<String>,
    /// Standalone statistics selection.
    pub operation: Option<String>,
}
/// Parse the strict committed fixture.
pub fn corpus() -> Vec<Row> {
    serde_json::from_str::<Corpus>(include_str!("../fixtures/session_documents.json"))
        .unwrap()
        .rows
}
/// Frame the supplied raw input, without normalizing authored bytes.
pub fn payload(input: &Input) -> String {
    input.base64.clone().unwrap_or_else(|| {
        let raw = input.raw.clone().unwrap_or_else(|| {
            input
                .data
                .clone()
                .unwrap_or_else(|| {
                    json!({
                        "header":{"id":"document","timestamp":"2026-10-15T00:00:00Z"},
                        "entries":[],"leafId":"default"
                    })
                })
                .to_string()
        });
        base64::engine::general_purpose::STANDARD.encode(raw)
    })
}
/// Call the public loading boundary.
pub fn load(row: &Row) -> std::io::Result<SessionDocument> {
    load_document(
        &payload(&row.input),
        row.input.query.as_deref().unwrap_or(""),
        row.input.injected.as_deref(),
    )
}
/// Retain just the named cases for a behavior witness.
pub fn cases(names: &[&str]) -> Vec<Row> {
    let rows: Vec<_> = corpus()
        .into_iter()
        .filter(|r| names.contains(&r.name.as_str()))
        .collect();
    assert_eq!(rows.len(), names.len());
    rows
}
/// Observe ordered location indexes through their retained source records.
pub fn indexes(doc: &SessionDocument) -> Value {
    let entries = &doc.data["entries"];
    json!({
        "byId": doc.by_id.iter().map(|(id, &position)| json!([id,entries[position]])).collect::<Vec<_>>(),
        "toolCallMap": doc.tool_call_map.iter().map(|(id, &(entry, content))| {
            let block = &entries[entry]["message"]["content"][content];
            let mut fields = serde_json::Map::new();
            for key in ["name", "arguments"] {
                if let Some(value) = block.get(key) { fields.insert(key.into(), value.clone()); }
            }
            json!([id, fields])
        }).collect::<Vec<_>>(),
        "labelMap": doc.label_map.iter().map(|(id, &position)| json!([id,entries[position]["label"]])).collect::<Vec<_>>()
    })
}
/// Observe every tree occurrence and verify its record's actual borrowed owner.
pub fn tree(doc: &SessionDocument) -> Value {
    fn node(doc: &SessionDocument, n: &maestro_app::SessionTreeNode<'_>) -> Value {
        let id = n.entry["id"].as_str().unwrap();
        assert!(std::ptr::eq(
            n.entry,
            std::ptr::from_ref(&doc.data["entries"][doc.by_id[id]])
        ));
        assert_eq!(n.label_timestamp, None);
        let mut out = json!({"entry": n.entry, "children": n.children.iter().map(|child| node(doc, child)).collect::<Vec<_>>()});
        if let Some(label) = n.label {
            out["label"] = label.clone();
        }
        out
    }
    Value::Array(doc.build_tree().iter().map(|n| node(doc, n)).collect())
}
/// Observe reported components without conflating their unused total slots.
pub fn stats(s: &maestro_web::export_html::template::Stats) -> Value {
    assert!(matches!(
        s.tokens.total_tokens,
        maestro_app::Presence::Present(total)
            if total.to_bits() == 0.0_f64.to_bits()
    ));
    assert_eq!(s.tokens.cost.total.to_bits(), 0.0_f64.to_bits());
    json!({"userMessages":s.user_messages,"assistantMessages":s.assistant_messages,
        "toolResults":s.tool_results,"customMessages":s.custom_messages,"compactions":s.compactions,
        "branchSummaries":s.branch_summaries,"toolCalls":s.tool_calls,
        "tokens":{"input":s.tokens.input,"output":s.tokens.output,"cacheRead":s.tokens.cache_read,"cacheWrite":s.tokens.cache_write},
        "cost":{"input":s.tokens.cost.input,"output":s.tokens.cost.output,"cacheRead":s.tokens.cost.cache_read,"cacheWrite":s.tokens.cost.cache_write},
        "models":s.models})
}
/// Compare canonical public JSON, including ordered array occurrences.
pub fn equal(actual: &Value, expected: &Value, name: &str) {
    assert_eq!(
        maestro_app::compact_json(actual).unwrap(),
        maestro_app::compact_json(expected).unwrap(),
        "{name}"
    );
}
/// Replay a complete corpus observation, preserving index order and tree occurrences.
pub fn replay(row: &Row) {
    let result = load(row);
    if row.branch == "error" {
        assert_eq!(
            result.unwrap_err().kind(),
            std::io::ErrorKind::InvalidData,
            "{}",
            row.name
        );
        assert_eq!(row.error_kind.as_deref(), Some("invalid_data"));
        assert!(row.output.is_none());
        if row.input.operation.as_deref() == Some("stats") {
            let entries = row.input.data.as_ref().unwrap()["entries"]
                .as_array()
                .unwrap();
            assert_eq!(
                maestro_web::export_html::template::compute_stats(entries)
                    .unwrap_err()
                    .kind(),
                std::io::ErrorKind::InvalidData
            );
        }
        return;
    }
    assert_eq!(row.branch, "success");
    assert!(row.error_kind.is_none());
    let doc = result.unwrap();
    let expected = row.output.as_ref().unwrap();
    if row.input.operation.as_deref() == Some("stats") {
        let entries = row.input.data.as_ref().unwrap()["entries"]
            .as_array()
            .unwrap();
        let computed = maestro_web::export_html::template::compute_stats(entries).unwrap();
        equal(&stats(&computed), expected, &row.name);
        equal(&stats(&doc.global_stats), expected, &row.name);
        return;
    }
    check_document(&doc, expected, &row.name);
}

/// Compare the complete prepared document projection with its recorded result.
fn check_document(doc: &SessionDocument, expected: &Value, name: &str) {
    let tree = tree(doc);
    if let Some(ids) = expected.get("treeIds") {
        assert_eq!(expected.as_object().unwrap().len(), 1);
        assert_eq!(tree.as_array().unwrap().len(), 1);
        assert_eq!(tree[0]["children"].as_array().unwrap().len(), 1);
        assert_eq!(
            &json!([
                tree[0]["entry"]["id"],
                [
                    tree[0]["children"][0]["entry"]["id"],
                    tree[0]["children"][0]["children"]
                ]
            ]),
            ids
        );
        return;
    }
    let mut actual = indexes(doc);
    actual["data"] = Value::Object(doc.data.clone());
    actual["urlTargetId"] = json!(doc.url_target_id);
    actual["globalStats"] = stats(&doc.global_stats);
    actual["tree"] = tree;
    match &doc.leaf_id {
        maestro_app::Presence::Missing => {}
        maestro_app::Presence::Null => actual["leafId"] = Value::Null,
        maestro_app::Presence::Present(value) => actual["leafId"] = value.clone(),
    }
    if expected.get("keys").is_some() {
        let raw = &doc.data["raw"];
        let compact = maestro_app::compact_json(raw).unwrap();
        let canonical: serde_json::Map<String, Value> = serde_json::from_str(&compact).unwrap();
        actual["keys"] = json!(canonical.keys().collect::<Vec<_>>());
        actual["negativeZero"] = json!(raw["zero"].as_f64().unwrap().is_sign_negative());
    }
    // Object member order is not a value distinction; ordered maps are projected as arrays.
    assert_eq!(
        actual.as_object().unwrap().len(),
        expected.as_object().unwrap().len()
    );
    for (key, value) in expected.as_object().unwrap() {
        equal(&actual[key], value, &format!("{name} {key}"));
    }
}
