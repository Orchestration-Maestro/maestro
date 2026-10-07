use maestro_models::repair_json;

#[test]
fn repair_controls_only_inside_strings() {
    for code in 0..32u8 {
        let c = char::from(code);
        let expected = match code {
            8 => "\\b".into(),
            12 => "\\f".into(),
            10 => "\\n".into(),
            13 => "\\r".into(),
            9 => "\\t".into(),
            _ => format!("\\u{code:04x}"),
        };
        assert_eq!(
            repair_json(&format!("{c}\"{c}\"{c}")),
            format!("{c}\"{expected}\"{c}")
        );
        assert_eq!(
            repair_json(&format!("\"\\\"{c}\"")),
            format!("\"\\\"{expected}\"")
        );
    }
}

#[test]
fn repair_backslashes_preserves_unicode_prefixes() {
    for escape in [
        r#"\""#,
        r"\\",
        r"\/",
        r"\b",
        r"\f",
        r"\n",
        r"\r",
        r"\t",
        r"\uAb09",
        r"\u12",
        r"\uGGGG",
        r"\uD83D\uDE00",
    ] {
        let input = format!("\"{escape}\"");
        assert_eq!(repair_json(&input), input);
    }
    for (input, expected) in [
        (r#""A\H""#, r#""A\\H""#),
        (r#""\q\n""#, r#""\\q\n""#),
        (r#""A\"#, r#""A\\"#),
    ] {
        assert_eq!(repair_json(input), expected);
    }
}

fn message(error: maestro_models::ThrownValue) -> String {
    maestro_models::format_thrown_value(&error).unwrap()
}
#[test]
fn full_parse_retries_only_changed_text() {
    use maestro_models::parse_json_with_repair as parse;
    assert_eq!(parse(r#"{"x":42}"#).unwrap(), serde_json::json!({"x":42}));
    assert_eq!(parse("\"a\tb\"").unwrap(), serde_json::json!("a\tb"));
    assert_eq!(parse(r#""A\H""#).unwrap(), serde_json::json!(r"A\H"));
    assert_eq!(
        message(parse("{").unwrap_err()),
        "Expected property name or '}' in JSON at position 1 (line 1 column 2)"
    );
    assert_eq!(
        message(parse("{\"x\":\"\t\",}").unwrap_err()),
        message(parse("{\"x\":\"\\t\",}").unwrap_err())
    );
}

#[test]
fn streaming_parse_keeps_fallback_order() {
    use maestro_models::parse_streaming_json as parse;
    use serde_json::json;
    for input in [None, Some(""), Some("\u{feff}"), Some("?")] {
        assert_eq!(parse(input), json!({}));
    }
    for (input, expected) in [
        ("null", json!(null)),
        ("nu", json!({})),
        ("false", json!(false)),
        ("0", json!(0)),
        (r#""""#, json!("")),
        ("[]", json!([])),
        (r#""A\H""#, json!(r"A\H")),
        (r#""A\H"#, json!("A")),
        ("\"a\tb", json!("a\tb")),
    ] {
        assert_eq!(parse(Some(input)), expected, "{input:?}");
    }
}

#[test]
fn partial_collections_keep_completed_prefixes() {
    use maestro_models::parse_streaming_json as parse;
    use serde_json::json;
    for (input, expected) in [
        ("[1,2,", json!([1, 2])),
        (r#"{"a":{"b":2,"c":"#, json!({"a":{"b":2}})),
        (r#"{"a":1,"b":"#, json!({"a":1})),
        (r#"{"a":1,"b"#, json!({"a":1})),
        (r#"{"a":1,}"#, json!({"a":1})),
        (r#"{"a":1,"b":?"#, json!({"a":1})),
        (r#"{"x" 1}"#, json!({})),
        (r#"{"x" 12}"#, json!({"x":2})),
        ("[1 2]", json!([])),
        ("true trailing", json!(true)),
        (r#"{"__proto__":{"x":1},"a":2"#, json!({"a":2})),
    ] {
        assert_eq!(parse(Some(input)), expected, "{input:?}");
    }
    let value = parse(Some(r#"{"z":0,"a":1,"z":2,"#));
    assert_eq!(value, json!({"z":2,"a":1}));
    assert_eq!(
        value.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["z", "a"]
    );
}

#[test]
fn partial_atoms_keep_literal_and_exponent_edges() {
    use maestro_models::parse_streaming_json as parse;
    use serde_json::json;
    for (input, expected) in [
        ("tr", json!(true)),
        ("fa", json!(false)),
        ("nu", json!({})),
        ("1e", json!(1)),
        ("1E", json!({})),
        ("123.", json!({})),
        ("-", json!({})),
        ("[1e,2]", json!([1, 2])),
        (r#""hi\u12"#, json!("hi")),
        (r#""hi\"#, json!("hi")),
        ("Na", json!({})),
        ("Inf", json!({})),
        ("-Inf", json!({})),
    ] {
        assert_eq!(parse(Some(input)), expected, "{input:?}");
    }
}

#[test]
fn partial_invalid_escape_can_drop_the_suffix() {
    use maestro_models::parse_streaming_json as parse;
    use serde_json::json;
    for (input, expected) in [
        (r#""A\H"#, json!("A")),
        (r#""A\H""#, json!(r"A\H")),
        (r#"{"x":"A\H"#, json!({"x":"A"})),
        (r#"{"x":"A\H"}"#, json!({"x":r"A\H"})),
    ] {
        assert_eq!(parse(Some(input)), expected);
    }
}

#[test]
fn json_text_preserves_utf16_boundaries() {
    use maestro_models::parse_json_with_repair as parse;
    use serde_json::json;
    for (input, expected) in [
        (r#""😀""#, json!("😀")),
        (r#""\uD83D\uDE00""#, json!("😀")),
        (r#""\uD800""#, json!("�")),
        (r#""\uDC00""#, json!("�")),
        (r#"{"\uD800":"😀\uDC00"}"#, json!({"�":"😀�"})),
    ] {
        assert_eq!(parse(input).unwrap(), expected);
        assert_eq!(repair_json(input), input);
    }
    assert_eq!(repair_json("\"😀\t\""), "\"😀\\t\"");
}

#[test]
fn surrogate_filter_removes_only_unpaired_units() {
    use maestro_models::sanitize_surrogates;
    for (input, expected) in [
        (vec![], ""),
        (vec![65, 0xd83d, 0xde00, 66], "A😀B"),
        (vec![0xd800, 65, 0xdc00], "A"),
        (vec![65, 0xd800, 66, 0xdc00, 67], "ABC"),
        (vec![0xd800, 0xd83d, 0xde00], "😀"),
        (vec![0xd83d, 0xde00, 0xdc00], "😀"),
        (vec![0xdc00, 0xd800, 65, 0xdc00, 0xd800], "A"),
        (vec![0xd800, 0xd83d, 0xde48], "🙈"),
    ] {
        assert_eq!(sanitize_surrogates(&input), expected);
    }
    assert_eq!(
        sanitize_surrogates(&"Hello 🌍".encode_utf16().collect::<Vec<_>>()),
        "Hello 🌍"
    );
}

#[test]
fn trim_and_json_whitespace_remain_distinct() {
    use maestro_models::parse_streaming_json as parse;
    use serde_json::json;
    let mut spaces = vec![
        9, 10, 11, 12, 13, 32, 160, 5760, 8232, 8233, 8239, 8287, 12288, 65279,
    ];
    spaces.extend(8192..=8202);
    for code in spaces {
        let c = char::from_u32(code).unwrap();
        assert_eq!(parse(Some(&format!("{c}42{c}"))), json!(42));
    }
    for c in ['\u{85}', '\u{180e}', '\u{200b}'] {
        assert_eq!(parse(Some(&format!("{c}42{c}"))), json!({}));
    }
    for c in [' ', '\n', '\r', '\t'] {
        assert_eq!(parse(Some(&format!("[1,{c}2]"))), json!([1, 2]));
    }
    assert_eq!(parse(Some("[1,\u{feff}2]")), json!([1]));
}

#[test]
fn parse_errors_keep_original_or_repaired_location() {
    let rows: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/json_errors.json")).unwrap();
    for row in rows.as_array().unwrap() {
        if let Some(expected) = row.get("final") {
            let error =
                maestro_models::parse_json_with_repair(row["input"].as_str().unwrap()).unwrap_err();
            let maestro_models::ThrownValue::Error(error) = error else {
                panic!("not an error");
            };
            assert_eq!(error.name, expected["name"]);
            assert_eq!(error.message, expected["message"], "{}", row["input"]);
        }
    }
}

#[test]
fn split_surrogate_escapes_reparse_cumulative_input() {
    use maestro_models::{parse_json_with_repair, parse_streaming_json as parse};
    use serde_json::json;
    for (input, expected) in [
        (r#""\uD83D"#, json!("�")),
        (r#""\uD83D\u"#, json!("�")),
        (r#""\uD83D\uDE00""#, json!("😀")),
    ] {
        assert_eq!(parse(Some(input)), expected);
    }
    for input in [r#""\uD800""#, r#""\uDC00""#] {
        assert_eq!(parse_json_with_repair(input).unwrap(), json!("�"));
    }
    assert_eq!(
        parse_json_with_repair(r#"{"\uD800":"\uDC00"}"#).unwrap(),
        json!({"�":"�"})
    );
}

#[test]
fn overflow_numbers_follow_parse_failure_fallbacks() {
    use maestro_models::{parse_json_with_repair, parse_streaming_json as parse};
    use serde_json::json;
    assert_eq!(
        message(parse_json_with_repair("1e400").unwrap_err()),
        "Number outside binary64 range in JSON at position 0 (line 1 column 1)"
    );
    for (input, expected) in [
        ("1e400", json!({})),
        ("[1,1e400,2", json!([1])),
        (r#"{"a":1,"b":1e400,"c":2"#, json!({"a":1})),
        ("Na", json!({})),
        ("Inf", json!({})),
        ("-Inf", json!({})),
    ] {
        assert_eq!(parse(Some(input)), expected, "{input}");
    }
}

#[test]
fn astral_raw_token_replaces_only_generated_lone_unit() {
    let rows: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/generated_messages.json")).unwrap();
    for row in rows.as_array().unwrap() {
        let maestro_models::ThrownValue::Error(error) =
            maestro_models::parse_json_with_repair(row["input"].as_str().unwrap()).unwrap_err()
        else {
            panic!("expected Error");
        };
        assert_eq!(error.name, row["name"]);
        assert_eq!(error.message, row["expected"]);
        assert!(error.message.contains("😀"));
        assert!(error.message.contains("'�'"));
    }
}
