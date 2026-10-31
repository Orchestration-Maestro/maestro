#[cfg(test)]
mod tests {
    use maestro_models::sanitize_surrogates;

    #[test]
    fn maestro_sanitizes_encoded_provider_text() {
        assert_eq!(
            sanitize_surrogates(&[0xd800, 65, 0xdc00, 0xd83d, 0xde00, 0xd83d, 0xde01]),
            "A😀😁"
        );
        let result = maestro_models::ToolResultMessage::<serde_json::Value> {
            tool_call_id: "c".into(),
            tool_name: "text".into(),
            content: vec![maestro_models::UserBlock::Text(
                maestro_models::TextContent::new(sanitize_surrogates(&[
                    65, 0xd800, 0xd83d, 0xde00,
                ])),
            )],
            details: None,
            is_error: false,
            timestamp: 0.0,
            usage: None,
        };
        assert_eq!(
            serde_json::to_value(result).unwrap()["content"],
            serde_json::json!([{"type":"text","text":"A😀"}])
        );
    }

    #[test]
    fn maestro_repairs_only_json_string_literals() {
        use maestro_models::repair_json;
        assert_eq!(repair_json(r#"{"x":"é\\\""}"#), r#"{"x":"é\\\""}"#);
        assert_eq!(repair_json("5"), "5");
        let controls: String = (0..32).map(char::from).collect();
        assert_eq!(repair_json(&controls), controls);
        let repaired = repair_json(&format!("\"{controls}\""));
        assert_eq!(serde_json::from_str::<String>(&repaired).unwrap(), controls);
        assert!(repaired.contains(r"\u0000"));
        assert!(repaired.contains(r"\b\t\n\u000b\f\r"));
    }

    #[test]
    fn maestro_keeps_escape_and_unicode_boundaries() {
        use maestro_models::repair_json;
        for input in [r#""\"\\\/\b\f\n\r\t\u1234""#, r#""\u12""#, r#""é""#] {
            assert_eq!(repair_json(input), input);
        }
        assert_eq!(repair_json(r#""a\H""#), r#""a\\H""#);
        assert_eq!(repair_json("\"a\\"), "\"a\\\\");
    }

    #[test]
    fn maestro_reports_last_strict_parse_error() {
        use maestro_models::{parse_json_with_repair, repair_json};
        let deep = format!("{}0{}", "[".repeat(128), "]".repeat(128));
        for input in [
            "not JSON",
            "{\"x\":\"a\n\" trailing",
            r#""\uD800" trailing"#,
            r#"{"n":1e400,"s":"\uD800"}"#,
            r#"{"s":"\uD800","n":1e400}"#,
            r#"{"n":1e400,]"#,
            "{\"n\":1e400,\"s\":\"a\n\" trailing",
            deep.as_str(),
        ] {
            let expected = serde_json::from_str::<serde_json::Value>(&repair_json(input))
                .unwrap_err()
                .to_string();
            assert_eq!(parse_json_with_repair(input).unwrap_err().message, expected);
        }
        assert!(
            serde_json::from_str::<serde_json::Value>("1e400")
                .unwrap_err()
                .to_string()
                .starts_with("number out of range at line ")
        );
        assert_eq!(parse_json_with_repair("5").unwrap(), serde_json::json!(5));
        let controls: String = (0..32).map(char::from).collect();
        for (input, expected) in [
            (
                r#"{"x":"é\\\""}"#.to_owned(),
                serde_json::json!({"x":"é\\\""}),
            ),
            (format!("\"{controls}\""), serde_json::json!(controls)),
            (
                r#""\"\\\/\b\f\n\r\t\u1234""#.to_owned(),
                serde_json::json!("\"\\/\u{8}\u{c}\n\r\tሴ"),
            ),
            (r#""é""#.to_owned(), serde_json::json!("é")),
            (r#""a\H""#.to_owned(), serde_json::json!("a\\H")),
        ] {
            assert_eq!(parse_json_with_repair(&input).unwrap(), expected, "{input}");
        }
        for (input, repaired) in [
            (controls.as_str(), controls.as_str()),
            (r#""\u12""#, r#""\u12""#),
            ("\"a\\", "\"a\\\\"),
        ] {
            let expected = serde_json::from_str::<serde_json::Value>(repaired)
                .unwrap_err()
                .to_string();
            assert_eq!(parse_json_with_repair(input).unwrap_err().message, expected);
        }
    }

    #[test]
    fn maestro_accepts_representable_discarded_surrogate_duplicates() {
        use maestro_models::parse_json_with_repair;
        for (input, expected) in [
            (r#"{"s":"\uD800","s":"ok"}"#, serde_json::json!({"s":"ok"})),
            (r#"{"s":"\uDC00","s":"ok"}"#, serde_json::json!({"s":"ok"})),
            (
                r#"{"s":"\uD800","n":1e400,"s":"ok"}"#,
                serde_json::json!({"s":"ok","n":null}),
            ),
        ] {
            assert_eq!(parse_json_with_repair(input).unwrap(), expected, "{input}");
        }
        for (input, prefix) in [
            (r#""\uD800""#, "unexpected end of hex escape at line "),
            (
                r#""\uDC00""#,
                "lone leading surrogate in hex escape at line ",
            ),
        ] {
            let native = serde_json::from_str::<serde_json::Value>(input).unwrap_err();
            assert!(native.to_string().starts_with(prefix));
            assert_eq!(
                parse_json_with_repair(input).unwrap_err().message,
                native.to_string()
            );
        }
    }

    #[test]
    fn maestro_parses_decimal_arguments_exactly() {
        use maestro_models::{parse_json_with_repair, parse_streaming_json};
        let ratio = 51.248_178_375_505_404_f64;
        let complete = parse_json_with_repair(r#"{"ratio":51.248178375505404}"#).unwrap();
        assert_eq!(
            complete["ratio"].as_f64().map(f64::to_bits),
            Some(ratio.to_bits())
        );
        let streamed = parse_streaming_json(Some(r#"{"ratio":51.248178375505404"#));
        assert_eq!(
            streamed["ratio"].as_f64().map(f64::to_bits),
            Some(ratio.to_bits())
        );
    }

    fn cut(json: &str, trailing_bytes: usize) -> &str {
        &json[..json.len() - trailing_bytes]
    }

    #[test]
    fn maestro_streamed_arguments_keep_partial_values() {
        use maestro_models::parse_streaming_json;
        use serde_json::json;
        for (input, expected) in [
            ("{", json!({})),
            (r#"{"a"#, json!({})),
            (r#"{"a":"#, json!({})),
            (r#"{"a":{"b":"#, json!({"a":{}})),
            (r#"{"a":["#, json!({"a":[]})),
            (r#"{"a":[1,"#, json!({"a":[1]})),
            (r#"{"a":[1,"é"#, json!({"a":[1,"é"]})),
            (cut(r#"{"a":true}"#, 2), json!({"a":true})),
            ("1e", json!(1)),
            ("1E", json!(1)),
            ("1.", json!(1)),
            ("{\"x\":\"a\nb", json!({})),
        ] {
            assert_eq!(parse_streaming_json(Some(input)), expected, "{input}");
        }
    }

    #[test]
    fn maestro_drops_unfinished_members_before_returning_prefix() {
        use maestro_models::parse_streaming_json;
        use serde_json::json;
        for (input, expected) in [
            (r#"{"a":1,"b":oops}"#, json!({"a":1})),
            (r#"{"n":{"a":"#, json!({"n":{}})),
            (r#"[{"a":1,"b":oops}]"#, json!([{"a":1}])),
        ] {
            assert_eq!(parse_streaming_json(Some(input)), expected, "{input}");
        }
    }

    #[test]
    fn maestro_keeps_completed_values_before_trailing_input() {
        use maestro_models::parse_streaming_json;
        use serde_json::json;
        for (input, expected) in [
            (r#"{"a":1} trailing"#, json!({"a":1})),
            ("[1] trailing", json!([1])),
            (r#""x" trailing"#, json!("x")),
            ("true trailing", json!(true)),
            ("5 trailing", json!({})),
            ("null trailing", json!({})),
            ("prose {\"a\":1}", json!({})),
        ] {
            assert_eq!(parse_streaming_json(Some(input)), expected, "{input}");
        }
    }

    #[test]
    fn maestro_truncates_incomplete_escape_at_its_start() {
        use maestro_models::parse_streaming_json;
        for input in [r#"{"a":"x\H"#, r#"{"a":"x\u12"#, r#"{"a":"x\"#] {
            assert_eq!(
                parse_streaming_json(Some(input)),
                serde_json::json!({"a":"x"}),
                "{input}"
            );
        }
        assert_eq!(
            parse_streaming_json(Some(r#"{"a":"x\u1234"}"#)),
            serde_json::json!({"a":"xሴ"})
        );
    }

    #[test]
    fn maestro_preserves_scalar_parse_results_and_blank_fallbacks() {
        use maestro_models::parse_streaming_json;
        use serde_json::json;
        for input in [None, Some(""), Some(" \t\r\n\u{feff}\u{a0}")] {
            assert_eq!(parse_streaming_json(input), json!({}));
        }
        for (input, expected) in [
            ("5", json!(5)),
            (r#""x""#, json!("x")),
            ("false", json!(false)),
            ("null", json!(null)),
            ("1e400", json!(null)),
            ("\u{feff}{\"a\":1", json!({"a":1})),
            ("\u{85}{\"a\":1", json!({})),
        ] {
            assert_eq!(parse_streaming_json(Some(input)), expected, "{input}");
        }
    }
}

#[test]
fn maestro_preserves_delimiters_inside_incomplete_strings() {
    assert_eq!(
        maestro_models::parse_streaming_json(Some("[\"hello]")),
        serde_json::json!(["hello]"])
    );
}

#[test]
fn maestro_keeps_completed_escapes_in_incomplete_strings() {
    assert_eq!(
        maestro_models::parse_streaming_json(Some("[\"x\\n")),
        serde_json::json!(["x\n"])
    );
}

#[test]
fn maestro_rejects_invalid_leading_json_grammar() {
    for input in ["1ee", "1..", "1e2."] {
        assert_eq!(
            maestro_models::parse_streaming_json(Some(input)),
            serde_json::json!({}),
            "{input}"
        );
    }
    assert_eq!(
        maestro_models::parse_streaming_json(Some(".5")),
        serde_json::json!({})
    );
    assert_eq!(
        maestro_models::parse_streaming_json(Some("01")),
        serde_json::json!({})
    );
}

#[test]
fn maestro_completes_literals_and_numeric_suffixes_consistently() {
    assert_eq!(
        maestro_models::parse_streaming_json(Some("9007199254740993e")),
        serde_json::json!(9_007_199_254_740_992_u64)
    );
    assert_eq!(
        maestro_models::parse_streaming_json(Some("1e+")),
        serde_json::json!(1)
    );
    assert_eq!(
        maestro_models::parse_streaming_json(Some("-1E-")),
        serde_json::json!(-1)
    );
}

#[test]
fn maestro_recovers_before_invalid_container_separators() {
    assert_eq!(
        maestro_models::parse_streaming_json(Some("[true false]")),
        serde_json::json!([true])
    );
    assert_eq!(
        maestro_models::parse_streaming_json(Some("{\"x\"=1}")),
        serde_json::json!({})
    );
    assert_eq!(
        maestro_models::parse_streaming_json(Some("[1 2]")),
        serde_json::json!([])
    );
}

#[test]
fn maestro_completes_streamed_argument_corpus() {
    let rows: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/streamed_arguments.json")).unwrap();
    for row in rows.as_array().unwrap() {
        let input = row["input"].as_str();
        let actual = maestro_models::parse_streaming_json(input);
        let expected = row["expected"].clone();
        assert_eq!(actual, expected, "{input:?}");
    }
}

#[test]
fn maestro_completes_every_proper_literal_prefix() {
    use maestro_models::parse_streaming_json;
    use serde_json::json;
    for (literal, root, array) in [
        ("true", json!(true), json!([true])),
        ("false", json!(false), json!([false])),
        ("null", json!({}), json!([null])),
        ("Infinity", json!({}), json!([])),
        ("-Infinity", json!({}), json!([])),
        ("NaN", json!({}), json!([])),
    ] {
        for length in 1..literal.len() {
            let prefix = &literal[..length];
            assert_eq!(parse_streaming_json(Some(prefix)), root, "{prefix}");
            let input = format!("[{prefix}");
            assert_eq!(parse_streaming_json(Some(&input)), array, "{input}");
        }
    }
}

/// Recorded number results at the three parsing and writing boundaries.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NumberCorpus {
    /// Complete values read by the strict parser.
    strict: Vec<NumberRow>,
    /// Accumulated prefixes read by the streaming parser.
    streaming: Vec<NumberRow>,
    /// Compact text cases consumed by the shared writer's unit test.
    compact: Vec<NumberRow>,
}

/// One input and its independent numeric result.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NumberRow {
    /// Original JSON text.
    input: String,
    /// Recorded projected value, or compact text in the writer corpus.
    expected: serde_json::Value,
    /// Recorded sign of the root number or the `n` member.
    negative_zero: Option<bool>,
}

/// Compare numeric leaves as doubles, independently of integer storage.
fn assert_numeric_value(actual: &serde_json::Value, expected: &serde_json::Value) {
    match (actual, expected) {
        (serde_json::Value::Number(a), serde_json::Value::Number(b)) => {
            assert_eq!(a.as_f64(), b.as_f64());
        }
        (serde_json::Value::Array(a), serde_json::Value::Array(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                assert_numeric_value(a, b);
            }
        }
        (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
            assert_eq!(a.len(), b.len());
            for (key, value) in b {
                assert!(a.contains_key(key), "missing member {key}");
                assert_numeric_value(&a[key], value);
            }
        }
        _ => assert_eq!(actual, expected),
    }
}

/// Check the recorded value and any recorded zero sign.
fn assert_number_row(row: &NumberRow, actual: &serde_json::Value) {
    assert_numeric_value(actual, &row.expected);
    if let Some(negative_zero) = row.negative_zero {
        let number = if actual.is_object() {
            &actual["n"]
        } else {
            actual
        };
        // Overflow has no numeric leaf in the projected JSON data model.
        if let Some(value) = number.as_f64() {
            assert_eq!(value == 0.0 && value.is_sign_negative(), negative_zero);
        } else {
            assert!(number.is_null());
            assert!(!negative_zero);
        }
    }
}

/// Load the typed fixture, including the rows owned by the writer test.
fn number_corpus() -> Result<NumberCorpus, serde_json::Error> {
    let corpus: NumberCorpus =
        serde_json::from_str(include_str!("fixtures/streamed_argument_numbers.json"))?;
    assert!(!corpus.compact.is_empty());
    Ok(corpus)
}

#[test]
fn maestro_projects_strict_argument_numbers() {
    for row in number_corpus().unwrap().strict {
        let actual = maestro_models::parse_json_with_repair(&row.input)
            .unwrap_or_else(|error| panic!("{}: {}", row.input, error.message));
        assert_number_row(&row, &actual);
    }
}

#[test]
fn maestro_projects_partial_argument_numbers() {
    for row in number_corpus().unwrap().streaming {
        let actual = maestro_models::parse_streaming_json(Some(&row.input));
        assert_number_row(&row, &actual);
    }
}

/// Construct JSON text without constructing a deeply nested owned value.
fn nested_arguments(kind: &str, depth: usize, leaf: &str, complete: bool) -> String {
    let openings: Vec<&str> = (0..depth)
        .map(|position| match kind {
            "array" => "[",
            "object" => r#"{"x":"#,
            _ if position % 2 == 0 => "[",
            _ => r#"{"x":"#,
        })
        .collect();
    let mut text = openings.concat();
    text.push_str(leaf);
    if complete {
        for open in openings.iter().rev() {
            text.push(if *open == "[" { ']' } else { '}' });
        }
    }
    text
}

/// Walk one-child containers to inspect the actual accepted leaf.
fn argument_leaf(mut value: &serde_json::Value, depth: usize) -> Option<&serde_json::Value> {
    for _ in 0..depth {
        value = match value {
            serde_json::Value::Array(items) => {
                assert_eq!(items.len(), 1);
                &items[0]
            }
            serde_json::Value::Object(members) => {
                assert_eq!(members.len(), 1);
                members.get("x")?
            }
            _ => return None,
        };
    }
    Some(value)
}

#[test]
fn maestro_bounds_materialized_arguments() {
    use maestro_models::{parse_json_with_repair, parse_streaming_json};
    let cases = ["array", "object", "mixed"].into_iter().flat_map(|kind| {
        [127, 128].into_iter().flat_map(move |depth| {
            ["0", "1e400"]
                .into_iter()
                .flat_map(move |leaf| [false, true].map(|complete| (kind, depth, leaf, complete)))
        })
    });
    for (kind, depth, leaf, complete) in cases {
        let text = nested_arguments(kind, depth, leaf, complete);
        let value = parse_streaming_json(Some(&text));
        if depth == 127 {
            let expected = if leaf == "0" {
                serde_json::json!(0)
            } else {
                serde_json::Value::Null
            };
            assert_eq!(argument_leaf(&value, depth).unwrap(), &expected);
        } else {
            assert_eq!(value, serde_json::json!({}));
        }
        let strict = parse_json_with_repair(&text);
        if depth == 127 && complete {
            assert_eq!(strict.unwrap(), value);
        } else {
            assert!(strict.is_err());
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_drops_extreme_argument_depth_safely() -> crate::chat::TestResult {
    if !maestro_test_process::selection().is_child() {
        return {
            let output = maestro_test_process::TestProcess::current(
                "maestro_drops_extreme_argument_depth_safely",
            )?
            .nocapture()
            .env_clear()
            .case("depth")
            .output()?;
            assert!(output.status.success(), "{output:?}");
            Ok(())
        };
    }
    for kind in ["array", "object", "mixed"] {
        for leaf in ["0", "1e400"] {
            for complete in [false, true] {
                let text = nested_arguments(kind, 100_000, leaf, complete);
                assert!(maestro_models::parse_json_with_repair(&text).is_err());
                let value = maestro_models::parse_streaming_json(Some(&text));
                assert_eq!(value, serde_json::json!({}));
                drop(value);
            }
        }
    }
    Ok(())
}

#[test]
fn maestro_counts_only_consumed_argument_containers() {
    use maestro_models::parse_streaming_json;
    use serde_json::json;
    let brackets = "[".repeat(128);
    let text = format!(r#"{{"brackets":"{brackets}","n":1e400}}"#);
    assert_eq!(
        parse_streaming_json(Some(&text)),
        json!({"brackets": brackets, "n": null})
    );
    let wide = format!("[{}]", vec!["[]"; 128].join(","));
    assert_eq!(
        parse_streaming_json(Some(&wide)),
        json!(vec![Vec::<u8>::new(); 128])
    );
    let trailing = format!(r#"{{"ok":1}} {brackets}"#);
    assert_eq!(parse_streaming_json(Some(&trailing)), json!({"ok": 1}));
}

#[test]
fn maestro_rejects_unrepresentable_argument_member_names() {
    for input in [
        r#"{"\ud800":7,"keep":1}"#,
        r#"{"nested":{"\udc00":7,"keep":1}}"#,
    ] {
        let native = serde_json::from_str::<serde_json::Value>(input).unwrap_err();
        assert_eq!(
            maestro_models::parse_json_with_repair(input)
                .unwrap_err()
                .message,
            native.to_string()
        );
    }
}

/// Both public paths retain object order and signed zero at the object-only seam.
#[test]
fn parsed_arguments_preserves_object_fallback_and_public_path() {
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/provider_local_rules.json")).unwrap();
    for row in rows.iter().filter(|row| row["kind"] == "arguments") {
        let input = row["input"]["text"].as_str().unwrap();
        for parse in [
            maestro_models::parsed_arguments,
            maestro_models::arguments::parsed_arguments,
        ] {
            let actual = parse(input);
            assert_numeric_value(&actual, &row["expected"]["value"]);
            let members = actual.as_object().expect("recorded object root");
            let keys: Vec<_> = members.keys().collect();
            assert_eq!(serde_json::to_value(keys).unwrap(), row["expected"]["keys"]);
            let negative_zero: Vec<_> = members
                .iter()
                .filter_map(|(key, value)| {
                    value
                        .as_f64()
                        .filter(|n| *n == 0.0 && n.is_sign_negative())
                        .map(|_| key)
                })
                .collect();
            assert_eq!(
                serde_json::to_value(negative_zero).unwrap(),
                row["expected"]["negativeZeroMembers"]
            );
        }
    }
}

/// Strict display parsing shares projection without model string repair.
#[test]
fn strict_json_projection_preserves_complete_values_without_repair() {
    use maestro_models::arguments::json_parse::parse_json;
    use serde_json::json;
    for input in ["1e309", "-1e309"] {
        assert_eq!(parse_json(input).unwrap(), serde_json::Value::Null);
    }
    assert_eq!(parse_json(r#"{"n":1e309}"#).unwrap(), json!({"n": null}));
    assert_eq!(parse_json("1e308").unwrap().as_f64(), Some(1e308));
    let duplicate = parse_json(r#"{"first":"\ud800","last":3,"first":7}"#).unwrap();
    assert_eq!(duplicate, json!({"first":7,"last":3}));
    assert_eq!(
        duplicate
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["first", "last"]
    );
    assert!(
        parse_json("-0")
            .unwrap()
            .as_f64()
            .unwrap()
            .is_sign_negative()
    );
    for input in [
        "\"raw\ncontrol\"",
        "\u{feff}null",
        "\u{85}null",
        "{} trailing",
        "[1,",
    ] {
        let native = serde_json::from_str::<serde_json::Value>(input).unwrap_err();
        assert_eq!(
            parse_json(input).unwrap_err().to_string(),
            native.to_string()
        );
    }
    assert_eq!(
        parse_json(" \r\n\t[false,0,\"\"] \n").unwrap(),
        json!([false, 0, ""])
    );
    assert_eq!(
        maestro_models::parse_json_with_repair("\"raw\ncontrol\"").unwrap(),
        json!("raw\ncontrol")
    );
}

/// One recorded tool-argument root input and its parsed value.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RootRow {
    /// Original argument text; absent when no text was supplied at all.
    input: Option<String>,
    /// Recorded root value.
    expected: serde_json::Value,
    /// Recorded sign of the root number.
    #[serde(default)]
    negative_zero: bool,
}

/// Recorded tool-argument root corpus; the validation section belongs to its own reader.
#[derive(serde::Deserialize)]
struct RootCorpus {
    /// Streaming parser rows.
    parser: Vec<RootRow>,
}

/// Check one recorded root value and its recorded zero sign.
fn assert_root_row(row: &RootRow, actual: &serde_json::Value) {
    assert_numeric_value(actual, &row.expected);
    match actual.as_f64() {
        Some(number) => assert_eq!(
            number == 0.0 && number.is_sign_negative(),
            row.negative_zero
        ),
        None => assert!(!row.negative_zero),
    }
}

/// The public streaming parser keeps every recorded root, not only objects.
#[test]
fn public_streaming_parser_keeps_recorded_roots() {
    let corpus: RootCorpus =
        serde_json::from_str(include_str!("fixtures/tool_argument_roots.json")).unwrap();
    assert_eq!(corpus.parser.len(), 37);
    for row in &corpus.parser {
        let actual = maestro_models::parse_streaming_json(row.input.as_deref());
        assert_root_row(row, &actual);
    }
}

/// Every public parsed-arguments path keeps the recorded root.
#[test]
fn parsed_arguments_keeps_every_recorded_root() {
    let corpus: RootCorpus =
        serde_json::from_str(include_str!("fixtures/tool_argument_roots.json")).unwrap();
    assert_eq!(corpus.parser.len(), 37);
    for row in &corpus.parser {
        let input = row.input.as_deref().unwrap_or_default();
        for parse in [
            maestro_models::parsed_arguments as fn(&str) -> serde_json::Value,
            maestro_models::arguments::parsed_arguments,
            maestro_models::arguments::json_parse::parsed_arguments,
        ] {
            assert_root_row(row, &parse(input));
        }
    }
}
