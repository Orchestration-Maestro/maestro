//! Shared tool record wire behavior.
use maestro_request::types::{TextContent, UserBlock};
use maestro_tool::tools::truncate::TruncationBy;
use maestro_tool::types::ToolExecutionMode;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A recorded decoder outcome.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// Whether decoding succeeds.
    ok: bool,
    /// Re-encoded successful value.
    value: Option<Value>,
}
/// One recorded query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Record vocabulary.
    kind: String,
    /// Exact decoder input.
    input: String,
    /// Recorded result.
    expected: Expected,
}
/// Compare a disjoint record group with the recorded baseline.
fn check<T: serde::de::DeserializeOwned + Serialize>(
    kind: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let cases: Vec<Case> = serde_json::from_str(include_str!("fixtures/record_cases.json"))?;
    let selected: Vec<_> = cases.iter().filter(|case| case.kind == kind).collect();
    assert!(!selected.is_empty());
    for (index, case) in selected.into_iter().enumerate() {
        let result = serde_json::from_str::<T>(&case.input);
        assert_eq!(result.is_ok(), case.expected.ok, "{kind} case {index}");
        if let Ok(value) = result {
            assert_value(
                &serde_json::to_value(value)?,
                case.expected
                    .value
                    .as_ref()
                    .ok_or("missing successful value")?,
            );
        } else {
            assert!(case.expected.value.is_none());
        }
    }
    Ok(())
}
/// Compare complete values, retaining numeric bits and collection order.
fn assert_value(actual: &Value, expected: &Value) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => {
            assert!(a.as_f64().is_some());
            assert_eq!(a.as_f64().map(f64::to_bits), b.as_f64().map(f64::to_bits));
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                assert_value(a, b);
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
            for (key, value) in a {
                assert_value(value, &b[key]);
            }
        }
        _ => assert_eq!(actual, expected),
    }
}
#[test]
fn tool_vocabularies_keep_wire_forms() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/record_cases.json")).unwrap();
    assert_eq!(cases.len(), 228);
    let mut queries = std::collections::HashSet::new();
    for case in &cases {
        assert!(
            ["mode", "limit", "agent", "truncation", "details", "bash"]
                .contains(&case.kind.as_str())
        );
        assert!(queries.insert((&case.kind, &case.input)));
    }
    check::<ToolExecutionMode>("mode").unwrap();
    check::<TruncationBy>("limit").unwrap();
}

#[test]
fn agent_result_fields_preserve_guest_presence() {
    check::<
        maestro_tool::types::AgentToolResult<
            maestro_request::presence::Presence<String>,
            maestro_request::presence::Presence<bool>,
        >,
    >("agent")
    .unwrap();
}

#[test]
fn text_result_builds_one_unsigned_text_block() {
    for details in [
        None,
        Some(Value::Null),
        Some(serde_json::json!({"answer": 42})),
    ] {
        let result = maestro_tool::types::AgentToolResult::<_, Option<bool>>::text_result(
            "first\n雪".into(),
            details.clone(),
        );
        assert_eq!(
            result.content,
            [UserBlock::Text(TextContent {
                text: "first\n雪".into(),
                text_signature: None,
            })]
        );
        assert_eq!(result.details, details);
        assert_eq!(result.terminate, None);
    }
}

#[test]
fn agent_result_native_details_remain_typed() {
    /// Caller data with no default value.
    #[derive(Serialize)]
    struct Details {
        /// A distinguishable detail.
        answer: u8,
    }
    let typed = maestro_tool::types::AgentToolResult {
        content: vec![],
        details: Some(Details { answer: 42 }),
        terminate: Some(false),
        usage: None,
    };
    assert_eq!(
        serde_json::to_value(typed).unwrap(),
        serde_json::json!({"content":[], "details":{"answer":42}, "terminate":false})
    );
    for (details, terminate, expected) in [
        (None, None, serde_json::json!({"content":[]})),
        (
            Some(Value::Null),
            Some(true),
            serde_json::json!({"content":[], "details":null, "terminate":true}),
        ),
    ] {
        let result = maestro_tool::types::AgentToolResult {
            content: vec![],
            details,
            terminate,
            usage: None,
        };
        assert_eq!(serde_json::to_value(result).unwrap(), expected);
    }
}

#[test]
fn truncation_views_keep_numeric_domains() {
    check::<maestro_tool::tools::truncate::TruncationResult<f64>>("truncation").unwrap();
    let native = maestro_tool::tools::truncate::TruncationResult {
        content: "native".into(),
        truncated: true,
        truncated_by: Some(TruncationBy::Lines),
        total_lines: usize::MAX,
        total_bytes: 2,
        output_lines: 3,
        output_bytes: 4,
        last_line_partial: false,
        first_line_exceeds_limit: true,
        max_lines: 5,
        max_bytes: 6,
    };
    assert_eq!(
        serde_json::to_value(native).unwrap(),
        serde_json::json!({
            "content":"native", "truncated":true, "truncatedBy":"lines", "totalLines":usize::MAX,
            "totalBytes":2, "outputLines":3, "outputBytes":4, "lastLinePartial":false,
            "firstLineExceedsLimit":true, "maxLines":5, "maxBytes":6
        })
    );
    for number in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let host = maestro_tool::tools::truncate::TruncationResult {
            content: String::new(),
            truncated: false,
            truncated_by: None,
            total_lines: number,
            total_bytes: number,
            output_lines: number,
            output_bytes: number,
            last_line_partial: false,
            first_line_exceeds_limit: false,
            max_lines: number,
            max_bytes: number,
        };
        let value = serde_json::to_value(host).unwrap();
        for field in [
            "totalLines",
            "totalBytes",
            "outputLines",
            "outputBytes",
            "maxLines",
            "maxBytes",
        ] {
            assert_eq!(value[field], Value::Null);
        }
    }
}

#[test]
fn read_details_keep_optional_truncation() {
    check::<
        maestro_tool::tools::read::ReadToolDetails<
            maestro_request::presence::Presence<
                maestro_tool::tools::truncate::TruncationResult<f64>,
            >,
        >,
    >("details")
    .unwrap();
    let absent = maestro_tool::tools::read::ReadToolDetails::<
        Option<maestro_tool::tools::truncate::TruncationResult<usize>>,
    > {
        truncation: None,
    };
    assert_eq!(serde_json::to_value(absent).unwrap(), serde_json::json!({}));
    let present = maestro_tool::tools::read::ReadToolDetails {
        truncation: Some(maestro_tool::tools::truncate::TruncationResult {
            content: "selected".into(),
            truncated: true,
            truncated_by: Some(TruncationBy::Bytes),
            total_lines: 11_usize,
            total_bytes: 22,
            output_lines: 3,
            output_bytes: 4,
            last_line_partial: true,
            first_line_exceeds_limit: false,
            max_lines: 5,
            max_bytes: 6,
        }),
    };
    assert_eq!(
        serde_json::to_value(present).unwrap(),
        serde_json::json!({"truncation":{
            "content":"selected", "truncated":true, "truncatedBy":"bytes", "totalLines":11,
            "totalBytes":22, "outputLines":3, "outputBytes":4, "lastLinePartial":true,
            "firstLineExceedsLimit":false, "maxLines":5, "maxBytes":6
        }})
    );
}

#[test]
fn bash_result_fields_keep_independent_presence() {
    check::<
        maestro_tool::bash_executor::BashResult<
            maestro_request::presence::Presence<f64>,
            maestro_request::presence::Presence<String>,
        >,
    >("bash")
    .unwrap();
}
