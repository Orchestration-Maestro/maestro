//! Native record admission and producer behavior.
#![cfg(not(target_arch = "wasm32"))]
use maestro_tools::{ReadToolInput, TruncationOptions};
use serde::Deserialize;
use serde_json::Value;

/// Recorded admission outcome.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// Decoder success.
    ok: bool,
    /// Successful field projection.
    value: Option<Value>,
}
/// Exact native query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadCase {
    /// Raw decoder input.
    input: String,
    /// Baseline outcome.
    expected: Expected,
}
#[test]
fn native_read_input_keeps_its_admission_policy() {
    let cases: Vec<ReadCase> =
        serde_json::from_str(include_str!("fixtures/native_read_cases.json")).unwrap();
    assert_eq!(cases.len(), 38);
    let mut queries = std::collections::HashSet::new();
    for (index, case) in cases.into_iter().enumerate() {
        assert!(queries.insert(case.input.clone()));
        let result = serde_json::from_str::<ReadToolInput>(&case.input);
        assert_eq!(result.is_ok(), case.expected.ok, "case {index}");
        if let Ok(input) = result {
            let expected = case.expected.value.unwrap();
            assert_eq!(input.path, expected["path"].as_str().unwrap());
            for (actual, key) in [(input.offset, "offset"), (input.limit, "limit")] {
                assert_eq!(
                    actual.map(f64::to_bits),
                    expected[key].as_f64().map(f64::to_bits),
                    "case {index} {key}"
                );
            }
        } else {
            assert!(case.expected.value.is_none());
        }
    }
}

/// Supplied producer budgets.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Options {
    /// Optional line limit.
    max_lines: Option<usize>,
    /// Optional byte limit.
    max_bytes: Option<usize>,
}
/// Recorded producer invocation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProducerCase {
    /// Assertion label.
    id: String,
    /// Head or tail selection.
    op: String,
    /// Authored text.
    text: String,
    /// Supplied budgets.
    options: Options,
    /// Complete expected record.
    expected: Value,
}
#[test]
fn native_bash_result_uses_shared_record() {
    let native = maestro_tools::BashResult {
        output: "literal output".into(),
        exit_code: Some(0.0),
        cancelled: false,
        truncated: false,
        full_output_path: None,
    };
    let shared: maestro_tool::bash_executor::BashResult<Option<f64>, Option<String>> = native;
    let result: maestro_tools::process::executor::BashResult = shared;
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        serde_json::json!({
            "output": "literal output", "exitCode": 0.0,
            "cancelled": false, "truncated": false
        })
    );
}

#[test]
fn native_record_aliases_keep_producer_fields() {
    let cases: Vec<ProducerCase> =
        serde_json::from_str(include_str!("fixtures/shared_producer_cases.json")).unwrap();
    assert_eq!(cases.len(), 24);
    let mut ids = std::collections::HashSet::new();
    for case in cases {
        assert!(ids.insert(case.id.clone()));
        let options = TruncationOptions {
            max_lines: case.options.max_lines,
            max_bytes: case.options.max_bytes,
        };
        let result: maestro_tools::output::limits::TruncationResult = match case.op.as_str() {
            "head" => maestro_tools::truncate_head(&case.text, options),
            "tail" => maestro_tools::truncate_tail(&case.text, options),
            _ => panic!("unknown producer"),
        };
        let shared: maestro_tool::tools::truncate::TruncationResult<usize> = result;
        let root: maestro_tools::TruncationResult = shared;
        let reason: Option<maestro_tool::tools::truncate::TruncationBy> = root.truncated_by;
        let result = maestro_tools::TruncationResult {
            truncated_by: reason,
            ..root
        };
        let details: maestro_tools::reading::ReadToolDetails = maestro_tools::ReadToolDetails {
            truncation: Some(result),
        };
        let shared: maestro_tool::tools::reading::ReadToolDetails<
            Option<maestro_tools::TruncationResult>,
        > = details;
        assert_eq!(
            serde_json::to_value(shared).unwrap(),
            serde_json::json!({"truncation":case.expected}),
            "{}",
            case.id
        );
    }
}
