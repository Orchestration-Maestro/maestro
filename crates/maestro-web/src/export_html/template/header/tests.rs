//! Recorded literal identity operands consumed by native mounting.
use super::*;
use base64::Engine as _;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    name: String,
    data: serde_json::Value,
    title: String,
}
fn document(value: &serde_json::Value) -> SessionDocument {
    super::super::load_document(
        &base64::engine::general_purpose::STANDARD.encode(value.to_string()),
        "",
        None,
    )
    .unwrap()
}
#[test]
fn header_identity_stays_literal_and_uses_truthy_fallback() {
    let rows: Vec<Row> = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/session_identity.json"
    ))
    .unwrap();
    assert_eq!(rows.len(), 6);
    let corpus = cases();
    for row in rows {
        let case = corpus.iter().find(|case| case.name == row.name).unwrap();
        assert_eq!(row.data, case.input["data"]);
        assert_eq!(row.title, case.output["title"]);
        assert_metadata(case);
    }
    let text = "  <script>雪 & \"quotes\"</script>  ";
    assert_eq!(
        prepare(
            &document(&json!({"entries":[],"header":{"id":text}})),
            &|_| unreachable!()
        )
        .unwrap()
        .title,
        format!("Session: {text}")
    );
    for id in [json!(true), json!(12), json!({}), json!([])] {
        assert_eq!(
            prepare(
                &document(&json!({"entries":[],"header":{"id":id}})),
                &|_| unreachable!()
            )
            .unwrap_err()
            .kind(),
            io::ErrorKind::InvalidData
        );
    }
}

#[test]
fn header_empty_uses_exact_fallbacks() {
    let row = cases().into_iter().find(|row| row.name == "empty").unwrap();
    let prepared = prepare(&case_document(&row), &|_| panic!("unexpected date")).unwrap();
    assert_prepared(&prepared, &row);
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    input: serde_json::Value,
    output: serde_json::Value,
}
fn cases() -> Vec<Case> {
    serde_json::from_str(include_str!(
        "../../../../tests/fixtures/session_header.json"
    ))
    .unwrap()
}
fn case_document(row: &Case) -> SessionDocument {
    let mut doc = document(&row.input["data"]);
    if let Some(stats) = row.input.get("stats") {
        let s = &mut doc.global_stats;
        for (field, key) in [
            (&mut s.user_messages, "userMessages"),
            (&mut s.assistant_messages, "assistantMessages"),
            (&mut s.tool_results, "toolResults"),
            (&mut s.custom_messages, "customMessages"),
            (&mut s.compactions, "compactions"),
            (&mut s.branch_summaries, "branchSummaries"),
            (&mut s.tool_calls, "toolCalls"),
        ] {
            if let Some(n) = stats[key].as_u64() {
                *field = usize::try_from(n).unwrap();
            }
        }
        if let Some(models) = stats["models"].as_array() {
            s.models = models
                .iter()
                .map(|v| v.as_str().unwrap().to_owned())
                .collect();
        }
        for (field, source, key) in [
            (&mut s.tokens.input, "tokens", "input"),
            (&mut s.tokens.output, "tokens", "output"),
            (&mut s.tokens.cache_read, "tokens", "cacheRead"),
            (&mut s.tokens.cache_write, "tokens", "cacheWrite"),
            (&mut s.tokens.cost.input, "cost", "input"),
            (&mut s.tokens.cost.output, "cost", "output"),
            (&mut s.tokens.cost.cache_read, "cost", "cacheRead"),
            (&mut s.tokens.cost.cache_write, "cost", "cacheWrite"),
            (&mut s.tokens.cost.total, "cost", "total"),
        ] {
            if let Some(n) = stats[source][key].as_f64() {
                *field = n;
            }
        }
    }
    doc
}
fn assert_metadata(row: &Case) {
    let doc = case_document(row);
    let prepared = prepare(&doc, &|_| {
        Ok(row.output["info"][0][1].as_str().unwrap().into())
    })
    .unwrap();
    assert_prepared(&prepared, row);
}
fn assert_prepared(prepared: &Prepared, row: &Case) {
    assert_eq!(prepared.title, row.output["title"], "{}", row.name);
    for ((label, value), expected) in LABELS
        .iter()
        .zip(&prepared.rows)
        .zip(row.output["info"].as_array().unwrap())
    {
        assert_eq!(json!([label, value]), *expected, "{}", row.name);
    }
    assert_eq!(HELP, row.output["help"]);
    for ((text, title, _), expected) in CONTROLS
        .iter()
        .zip(row.output["buttons"].as_array().unwrap())
    {
        assert_eq!(json!({"text":text,"title":title}), *expected);
    }
}
#[test]
fn header_categories_keep_order_and_omit_only_zero() {
    for row in cases().iter().filter(|r| {
        r.name.starts_with("counter-")
            || [
                "populated",
                "all-categories",
                "no-models",
                "empty-model-name",
                "all-negative-tokens",
            ]
            .contains(&r.name.as_str())
    }) {
        assert_metadata(row);
    }
}

#[test]
fn header_uses_prepared_statistics_without_recounting() {
    let row = cases()
        .into_iter()
        .find(|r| r.name == "stats-snapshot")
        .unwrap();
    assert_metadata(&row);
    let mut doc = case_document(&row);
    doc.data["entries"] = json!([{"type":"message","message":false}]);
    doc.data
        .insert("systemPrompt".into(), json!("admitted prompt"));
    doc.data.insert("tools".into(), json!(false));
    let prepared = prepare(&doc, &|_| unreachable!()).unwrap();
    assert_eq!(
        prepared.rows,
        [
            "unknown",
            "z/<tag>, a, z/<tag>",
            "7 user",
            "9",
            "↑-2 ↓3 R4 W5",
            "$0.033"
        ]
    );
}

#[test]
fn header_dates_forward_values_lazily_and_keep_localized_text() {
    use std::cell::RefCell;
    for row in cases()
        .iter()
        .filter(|r| r.input["operation"] == "date" || r.name.starts_with("date-"))
    {
        let data =
            row.input.get("data").cloned().unwrap_or_else(
                || json!({"entries":[],"header":{"timestamp":row.input["timestamp"]}}),
            );
        let doc = document(&data);
        let seen = RefCell::new(Vec::new());
        let localized = if row.output.is_string() {
            row.output.as_str().unwrap()
        } else {
            row.output["info"][0][1].as_str().unwrap()
        };
        let callback = |value: &serde_json::Value| {
            seen.borrow_mut().push(value.clone());
            Ok(localized.into())
        };
        let prepared = prepare(&doc, &callback).unwrap();
        if row.output.is_object() {
            assert_prepared(&prepared, row);
        }
        let timestamp = &data["header"]["timestamp"];
        if maestro_app::is_truthy(timestamp) {
            assert_eq!(seen.borrow().as_slice(), std::slice::from_ref(timestamp));
            assert_eq!(prepared.rows[0], localized);
            let error = prepare(&doc, &|_| {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "date failed",
                ))
            })
            .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        } else {
            assert!(seen.borrow().is_empty());
            assert_eq!(prepared.rows[0], "unknown");
        }
    }
    let literal = document(&json!({"entries":[],"header":{"timestamp":1}}));
    assert_eq!(
        prepare(&literal, &|_| Ok("  native <雪>  ".into()))
            .unwrap()
            .rows[0],
        "  native <雪>  "
    );
    let doc = document(&json!({"entries":[],"header":{"id":true,"timestamp":"truthy"}}));
    assert_eq!(
        prepare(&doc, &|_| panic!("identity must fail first"))
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidData
    );
}

#[test]
fn header_tokens_keep_thousand_and_million_rounding() {
    let rows = cases();
    let rows: Vec<_> = rows
        .iter()
        .filter(|r| r.input["operation"] == "tokens")
        .collect();
    assert_eq!(rows.len(), 28);
    for row in rows {
        let count: f64 = row.input["count"].as_str().unwrap().parse().unwrap();
        assert_eq!(format_tokens(count), row.output, "{}", row.name);
    }
}

#[test]
fn header_cost_sums_four_components_before_three_place_rounding() {
    for row in cases().iter().filter(|r| {
        [
            "stats-snapshot",
            "cost-cancellation",
            "negative-cost",
            "fixed-half",
            "large-cost",
        ]
        .contains(&r.name.as_str())
    }) {
        assert_metadata(row);
    }
}

mod disclosures;
