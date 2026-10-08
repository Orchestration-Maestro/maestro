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
                maestro_models::TextContent {
                    text: sanitize_surrogates(&[65, 0xd800, 0xd83d, 0xde00]),
                    text_signature: None,
                },
            )],
            details: None,
            is_error: false,
            timestamp: 0.0,
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
        for input in ["not JSON", "{\"x\":\"a\n\" trailing"] {
            let expected = serde_json::from_str::<serde_json::Value>(&repair_json(input))
                .unwrap_err()
                .to_string();
            assert_eq!(parse_json_with_repair(input).unwrap_err().message, expected);
        }
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
            ("1e400", json!({})),
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
        serde_json::json!(9_007_199_254_740_993_u64)
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
