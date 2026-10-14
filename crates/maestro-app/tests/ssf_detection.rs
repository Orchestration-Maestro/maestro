//! Spreadsheet date-format detection.
use maestro_app::browser::ssf::is_date;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Query {
    id: String,
    group: String,
    operation: String,
    args: serde_json::Value,
    expected: serde_json::Value,
}

#[test]
fn date_detection_keeps_literals_numeric_runs_and_calendar_markers() {
    let queries: Vec<Query> =
        serde_json::from_str(include_str!("fixtures/ssf/evaluator.json")).unwrap();
    let mut count = 0;
    for query in queries
        .into_iter()
        .filter(|q| q.group.starts_with("date_detection"))
    {
        assert_eq!(query.operation, "is_date");
        let format = query.args["format"].as_str().unwrap();
        assert_eq!(query.args.as_object().unwrap().len(), 1);
        assert_eq!(
            serde_json::json!({"ok": is_date(format)}),
            query.expected,
            "{}",
            query.id
        );
        count += 1;
    }
    assert_eq!(count, 61);
}
