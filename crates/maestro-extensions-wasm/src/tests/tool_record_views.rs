//! Guest tool record identity and read-input admission.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::Presence;

#[test]
fn guest_tool_aliases_use_shared_records() {
    let result: crate::AgentToolResult = maestro_tool::types::AgentToolResult {
        content: vec![],
        details: Presence::Null,
        terminate: Presence::Present(false),
        usage: None,
    };
    let module: crate::agent::AgentToolResult = result;
    assert!(matches!(module.details, Presence::Null));
    let mode: crate::ToolExecutionMode = maestro_tool::types::ToolExecutionMode::Parallel;
    assert_eq!(serde_json::to_value(mode).unwrap(), "parallel");
    let truncation: crate::TruncationResult = maestro_tool::tools::truncate::TruncationResult {
        content: "guest".into(),
        truncated: true,
        truncated_by: Some(crate::TruncationLimit::Bytes),
        total_lines: 1.25,
        total_bytes: 2.5,
        output_lines: 3.75,
        output_bytes: 4.5,
        last_line_partial: true,
        first_line_exceeds_limit: false,
        max_lines: 5.25,
        max_bytes: 6.5,
    };
    let module: crate::tools::TruncationResult = truncation;
    let reason: Option<maestro_tool::tools::truncate::TruncationBy> = module.truncated_by;
    let details: crate::ReadToolDetails = maestro_tool::tools::read::ReadToolDetails {
        truncation: Presence::Present(crate::TruncationResult {
            truncated_by: reason,
            ..module
        }),
    };
    let module: crate::tools::ReadToolDetails = details;
    assert!(matches!(module.truncation, Presence::Present(_)));
    let bash: crate::BashResult = maestro_tool::bash_executor::BashResult {
        output: "output".into(),
        exit_code: Presence::Present(-0.0),
        cancelled: false,
        truncated: true,
        full_output_path: Presence::Missing,
    };
    let module: crate::bash_executor::BashResult = bash;
    assert!(
        matches!(module.exit_code, Presence::Present(code) if code.to_bits() == (-0.0_f64).to_bits())
    );
}

/// Recorded guest admission outcome.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// Decoder success.
    ok: bool,
    /// Complete re-encoded record.
    value: Option<serde_json::Value>,
}
/// Exact guest read query.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadCase {
    /// Raw input.
    input: String,
    /// Baseline result.
    expected: Expected,
}
/// Select disjoint object and nonobject fixture groups.
fn read_cases(objects: bool) -> Result<Vec<ReadCase>, serde_json::Error> {
    let cases: Vec<ReadCase> =
        serde_json::from_str(include_str!("../../tests/fixtures/read_input_cases.json"))?;
    assert_eq!(cases.len(), 38);
    let mut queries = std::collections::HashSet::new();
    for case in &cases {
        assert!(queries.insert(&case.input));
    }
    let selected: Vec<_> = cases
        .into_iter()
        .filter(|case| case.input.trim_start().starts_with('{') == objects)
        .collect();
    assert!(!selected.is_empty());
    Ok(selected)
}
#[test]
fn guest_read_input_retains_presence_and_extensions() {
    for (index, case) in read_cases(true).unwrap().into_iter().enumerate() {
        let result = serde_json::from_str::<crate::ReadToolInput>(&case.input);
        assert_eq!(result.is_ok(), case.expected.ok, "case {index}");
        if let Ok(input) = result {
            let expected = case.expected.value.unwrap();
            let expected_extra: Vec<_> = expected
                .as_object()
                .unwrap()
                .keys()
                .filter(|key| !["path", "offset", "limit"].contains(&key.as_str()))
                .collect();
            assert_eq!(
                input.extra.keys().collect::<Vec<_>>(),
                expected_extra,
                "case {index}"
            );
            for (actual, key) in [(&input.offset, "offset"), (&input.limit, "limit")] {
                assert_eq!(
                    serde_json::to_value(actual)
                        .ok()
                        .and_then(|value| value.as_f64())
                        .map(f64::to_bits),
                    expected
                        .get(key)
                        .and_then(serde_json::Value::as_f64)
                        .map(f64::to_bits),
                    "case {index} {key}"
                );
            }
            assert_eq!(
                serde_json::to_value(input).unwrap(),
                expected,
                "case {index}"
            );
        } else {
            assert!(case.expected.value.is_none());
        }
    }
}
#[test]
fn guest_read_input_keeps_object_only_admission() {
    for case in read_cases(false).unwrap() {
        assert!(!case.expected.ok);
        assert!(case.expected.value.is_none());
        assert!(serde_json::from_str::<crate::ReadToolInput>(&case.input).is_err());
    }
}
