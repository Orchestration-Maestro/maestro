use maestro_resources::{
    FrontmatterError, FrontmatterValue, ParsedFrontmatter, parse_frontmatter, strip_frontmatter,
};

/// Keep literal scalar newlines separate from body trimming.
#[test]
fn maestro_frontmatter_preserves_literal_newlines() {
    let parsed =
        parse_frontmatter("---\ndescription: |\n  Line one\n  Line two\n---\n\nBody").unwrap();
    assert_eq!(
        parsed.frontmatter,
        FrontmatterValue::Mapping(vec![(
            "description".into(),
            FrontmatterValue::String("Line one\nLine two\n".into())
        )])
    );
    assert_eq!(parsed.body, "Body");
}

/// Leave ordinary and unterminated documents intact.
#[test]
fn maestro_frontmatter_keeps_unclosed_or_absent_headers() {
    for input in [
        "Just text\nsecond line",
        "---\nname: test\nBody without terminator",
        " ---\nname: no\n---\nbody",
    ] {
        let parsed = parse_frontmatter(input).unwrap();
        assert_eq!(parsed.body, input);
        assert_eq!(parsed.frontmatter, FrontmatterValue::Mapping(vec![]));
    }
}

/// Default only empty and null roots, retaining useful scalar kinds.
#[test]
fn maestro_frontmatter_defaults_empty_and_null_roots() {
    for yaml in ["", "# just a comment\n", "null\n"] {
        assert_eq!(
            parse_frontmatter(&format!("---\n{yaml}---\n Body "))
                .unwrap()
                .frontmatter,
            FrontmatterValue::Mapping(vec![])
        );
    }
    for (yaml, value) in [
        ("false", FrontmatterValue::Bool(false)),
        ("42", FrontmatterValue::Number(42.0)),
        (
            "[a, b]",
            FrontmatterValue::Sequence(vec![
                FrontmatterValue::String("a".into()),
                FrontmatterValue::String("b".into()),
            ]),
        ),
    ] {
        assert_eq!(
            parse_frontmatter(&format!("---\n{yaml}\n---\nbody"))
                .unwrap()
                .frontmatter,
            value
        );
    }
}

/// Strip through the parsing interface without trimming plain documents.
#[test]
fn maestro_frontmatter_strips_only_present_headers() {
    assert_eq!(
        strip_frontmatter("---\nkey: value\n---\n\nBody\n").unwrap(),
        "Body"
    );
    assert_eq!(
        strip_frontmatter("\n  No frontmatter body  \n").unwrap(),
        "\n  No frontmatter body  \n"
    );
}

/// Consume one complete opening character and retain closing suffixes.
#[test]
fn maestro_frontmatter_keeps_intact_delimiter_suffixes() {
    for input in [
        "---Xname: ok\n---suffix\n body ",
        "---😀name: ok\n---suffix\n body ",
    ] {
        let parsed = parse_frontmatter(input).unwrap();
        assert_eq!(
            parsed.frontmatter,
            FrontmatterValue::Mapping(vec![("name".into(), FrontmatterValue::String("ok".into()))])
        );
        assert_eq!(parsed.body, "suffix\n body");
    }
    assert_eq!(
        parse_frontmatter("---\nname: ok\n---suffix\n---\nnext")
            .unwrap()
            .body,
        "suffix\n---\nnext"
    );
}

/// Distinguish BOM whitespace from a retained next-line character.
#[test]
fn maestro_frontmatter_uses_exact_text_whitespace() {
    assert_eq!(
        parse_frontmatter("---\nx: y\n---\n\u{feff} body \u{feff}")
            .unwrap()
            .body,
        "body"
    );
    assert_eq!(
        parse_frontmatter("---\nx: y\n---\n\u{85}body\u{85}")
            .unwrap()
            .body,
        "\u{85}body\u{85}"
    );
    assert_eq!(
        parse_frontmatter("\u{feff} text \u{feff}").unwrap().body,
        "\u{feff} text \u{feff}"
    );
}

/// Resolve tagged scalars and anchored data without expanding merge keys.
#[test]
fn maestro_frontmatter_resolves_core_tags_and_aliases() {
    let parsed = parse_frontmatter("---\na: &a hello\nb: *a\nc: !!int \"42\"\nd: !!int 0x10\ne: !!float 1\nf: !!str 42\ng: !!bool TRUE\nh: !!null null\ni: !custom true\nbase: &base {description: shared}\n<<: *base\n---\nbody").unwrap();
    let shared = FrontmatterValue::Mapping(vec![(
        "description".into(),
        FrontmatterValue::String("shared".into()),
    )]);
    assert_eq!(
        parsed.frontmatter,
        FrontmatterValue::Mapping(vec![
            ("a".into(), FrontmatterValue::String("hello".into())),
            ("b".into(), FrontmatterValue::String("hello".into())),
            ("c".into(), FrontmatterValue::Number(42.0)),
            ("d".into(), FrontmatterValue::Number(16.0)),
            ("e".into(), FrontmatterValue::String("1".into())),
            ("f".into(), FrontmatterValue::String("42".into())),
            ("g".into(), FrontmatterValue::Bool(true)),
            ("h".into(), FrontmatterValue::Null),
            ("i".into(), FrontmatterValue::String("true".into())),
            ("base".into(), shared.clone()),
            ("<<".into(), shared),
        ])
    );
}

/// Reject equal typed keys and invalid document continuation.
#[test]
fn maestro_frontmatter_rejects_duplicate_keys_and_documents() {
    for yaml in [
        "description: first\ndescription: second",
        "1: first\n1: second",
        ".nan: first\n.nan: second",
        "a: b\n...\nc: d",
    ] {
        assert!(
            parse_frontmatter(&format!("---\n{yaml}\n---\nbody")).is_err(),
            "{yaml}"
        );
    }
}

/// Preserve insertion position when distinct keys stringify alike.
#[test]
fn maestro_frontmatter_retains_distinct_typed_key_overwrites() {
    let parsed =
        parse_frontmatter("---\n1: first\nx: middle\n\"1\": second\n? null\n: empty\n---\nbody")
            .unwrap();
    assert_eq!(
        parsed.frontmatter,
        FrontmatterValue::Mapping(vec![
            ("1".into(), FrontmatterValue::String("second".into())),
            ("x".into(), FrontmatterValue::String("middle".into())),
            (String::new(), FrontmatterValue::String("empty".into()))
        ])
    );
}

/// Retain f64 scalar kinds and exact scalar-key spellings.
#[test]
fn maestro_frontmatter_keeps_large_and_nonfinite_numbers() {
    let parsed = parse_frontmatter("---\nn: 18446744073709551616\nover: 1e400\nfraction: .5e2\npositive: .inf\nnegative: -.inf\nnan: .nan\n1e21: a\n1e20: b\n1e-6: c\n1e-7: d\n-0: e\n.inf: f\n-.inf: g\n.nan: h\n---\nbody").unwrap();
    let FrontmatterValue::Mapping(values) = parsed.frontmatter else {
        panic!("mapping expected");
    };
    assert_eq!(
        values[0].1,
        FrontmatterValue::Number(18_446_744_073_709_552_000.0)
    );
    assert_eq!(values[1].1, FrontmatterValue::Number(f64::INFINITY));
    assert_eq!(values[2].1, FrontmatterValue::Number(50.0));
    assert_eq!(values[3].1, FrontmatterValue::Number(f64::INFINITY));
    assert_eq!(values[4].1, FrontmatterValue::Number(f64::NEG_INFINITY));
    assert!(matches!(values[5].1, FrontmatterValue::Number(n) if n.is_nan()));
    assert_eq!(
        values[6..]
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        [
            "1e+21",
            "100000000000000000000",
            "0.000001",
            "1e-7",
            "0",
            "Infinity",
            "-Infinity",
            "NaN"
        ]
    );
}

/// Retain underlying text rather than a foreign binary object.
#[test]
fn maestro_frontmatter_resolves_binary_as_core_data() {
    assert_eq!(
        parse_frontmatter("---\nvalue: !!binary true\n---\nbody")
            .unwrap()
            .frontmatter,
        FrontmatterValue::Mapping(vec![("value".into(), FrontmatterValue::Bool(true))])
    );
    assert_eq!(
        parse_frontmatter("---\nblob: !!binary SGVsbG8=\n---\nbody")
            .unwrap()
            .frontmatter,
        FrontmatterValue::Mapping(vec![(
            "blob".into(),
            FrontmatterValue::String("SGVsbG8=".into())
        )])
    );
}

/// Retain underlying set-tagged mapping data.
#[test]
fn maestro_frontmatter_resolves_set_as_core_data() {
    assert_eq!(
        parse_frontmatter("---\nmembers: !!set { a: null, b: null }\n---\nbody")
            .unwrap()
            .frontmatter,
        FrontmatterValue::Mapping(vec![(
            "members".into(),
            FrontmatterValue::Mapping(vec![
                ("a".into(), FrontmatterValue::Null),
                ("b".into(), FrontmatterValue::Null)
            ])
        )])
    );
}

/// Retain timestamp-tagged text without a native date object.
#[test]
fn maestro_frontmatter_resolves_timestamp_as_core_data() {
    assert_eq!(
        parse_frontmatter("---\ndate: !!timestamp 2026-01-01\n---\nbody")
            .unwrap()
            .frontmatter,
        FrontmatterValue::Mapping(vec![(
            "date".into(),
            FrontmatterValue::String("2026-01-01".into())
        )])
    );
}

/// Retain the ordered-map tag's underlying sequence.
#[test]
fn maestro_frontmatter_resolves_omap_as_core_data() {
    assert_eq!(
        parse_frontmatter("---\norder: !!omap [a: 1, b: 2]\n---\nbody")
            .unwrap()
            .frontmatter,
        FrontmatterValue::Mapping(vec![(
            "order".into(),
            FrontmatterValue::Sequence(vec![
                FrontmatterValue::Mapping(vec![("a".into(), FrontmatterValue::Number(1.0))]),
                FrontmatterValue::Mapping(vec![("b".into(), FrontmatterValue::Number(2.0))])
            ])
        )])
    );
}

/// Retain the pairs tag's underlying sequence.
#[test]
fn maestro_frontmatter_resolves_pairs_as_core_data() {
    assert_eq!(
        parse_frontmatter("---\npairs: !!pairs [a: 1, b: 2]\n---\nbody")
            .unwrap()
            .frontmatter,
        FrontmatterValue::Mapping(vec![(
            "pairs".into(),
            FrontmatterValue::Sequence(vec![
                FrontmatterValue::Mapping(vec![("a".into(), FrontmatterValue::Number(1.0))]),
                FrontmatterValue::Mapping(vec![("b".into(), FrontmatterValue::Number(2.0))])
            ])
        )])
    );
}

/// Discard collection-key pairs without losing ordinary fields.
#[test]
fn maestro_frontmatter_ignores_collection_key_pairs() {
    for key in ["[a, b]", "[]", "[\"a, b\", \"true\"]", "{a: b}"] {
        assert_eq!(
            parse_frontmatter(&format!(
                "---\n? {key}\n: discarded\ndescription: kept\n---\nbody"
            ))
            .unwrap()
            .frontmatter,
            FrontmatterValue::Mapping(vec![(
                "description".into(),
                FrontmatterValue::String("kept".into())
            )])
        );
    }
}

/// Return the native parser cause through both parsing entry points.
#[test]
fn maestro_frontmatter_reports_invalid_metadata() {
    let input = "---\nfoo: [bar\n---\nBody";
    let error = parse_frontmatter(input).unwrap_err();
    assert!(!error.message.is_empty());
    assert!(error.line.is_some());
    assert!(error.column.is_some());
    assert_eq!(strip_frontmatter(input).unwrap_err(), error);
}

/// Normalize both authored carriage-return conventions.
#[test]
fn maestro_frontmatter_normalizes_line_endings() {
    for input in [
        "---\r\nname: test\r\n---\r\nLine one\r\nLine two",
        "---\rname: test\r---\rLine one\rLine two",
    ] {
        assert_eq!(parse_frontmatter(input).unwrap().body, "Line one\nLine two");
    }
}

/// Read quoted and hyphenated keys with the exact extracted body.
#[test]
fn maestro_frontmatter_reads_metadata_and_body() {
    let parsed = parse_frontmatter(
        "---\nname: \"skill-name\"\ndescription: 'A desc'\nfoo-bar: value\n---\n\nBody text",
    )
    .unwrap();
    assert_eq!(parsed.body, "Body text");
    assert_eq!(
        parsed.frontmatter,
        FrontmatterValue::Mapping(vec![
            ("name".into(), FrontmatterValue::String("skill-name".into())),
            (
                "description".into(),
                FrontmatterValue::String("A desc".into())
            ),
            ("foo-bar".into(), FrontmatterValue::String("value".into())),
        ])
    );
}

/// Signs inside radix prefixes remain strings in plain and tagged scalars.
#[test]
fn maestro_frontmatter_rejects_inner_radix_signs() {
    for scalar in ["0x+10", "0o+10", "0x-10", "0o-10", "0x", "0o", "0xg", "0o8"] {
        for tag in ["", "!!int "] {
            let parsed = parse_frontmatter(&format!("---\nn: {tag}{scalar}\n---")).unwrap();
            assert_eq!(
                parsed.frontmatter,
                FrontmatterValue::Mapping(vec![(
                    "n".into(),
                    FrontmatterValue::String(scalar.into())
                )]),
                "{tag}{scalar}"
            );
        }
    }
}

/// Radix integers have no intermediate width limit and round ties to even.
#[test]
fn maestro_frontmatter_rounds_unbounded_radix_numbers() {
    for (scalar, expected) in [
        (
            "0x100000000000000000000000000000000",
            3.402_823_669_209_385e38,
        ),
        (
            "0o4000000000000000000000000000000000000000000",
            3.402_823_669_209_385e38,
        ),
        ("0x20000000000001", 9_007_199_254_740_992.0),
        ("0x20000000000003", 9_007_199_254_740_996.0),
        ("0o400000000000000001", 9_007_199_254_740_992.0),
        ("0o400000000000000003", 9_007_199_254_740_996.0),
    ] {
        for tag in ["", "!!int "] {
            let parsed = parse_frontmatter(&format!("---\nn: {tag}{scalar}\n---")).unwrap();
            assert_eq!(
                parsed.frontmatter,
                FrontmatterValue::Mapping(vec![("n".into(), FrontmatterValue::Number(expected))]),
                "{tag}{scalar}"
            );
        }
    }
    for (prefix, digit) in [("0x", 'f'), ("0o", '7')] {
        for tag in ["", "!!int "] {
            let scalar = format!("{prefix}{}", digit.to_string().repeat(400));
            let parsed = parse_frontmatter(&format!("---\nn: {tag}{scalar}\n---")).unwrap();
            assert_eq!(
                parsed.frontmatter,
                FrontmatterValue::Mapping(vec![(
                    "n".into(),
                    FrontmatterValue::Number(f64::INFINITY)
                )])
            );
        }
    }
}

/// Core scalar spellings resolve to typed values while lookalikes remain text.
#[test]
fn maestro_frontmatter_resolves_schema_scalar_spellings() {
    for (yaml, expected) in [
        ("0o17", FrontmatterValue::Number(15.0)),
        ("-12", FrontmatterValue::Number(-12.0)),
        ("+12", FrontmatterValue::Number(12.0)),
        (".5", FrontmatterValue::Number(0.5)),
        ("12.", FrontmatterValue::Number(12.0)),
        ("1.5e+2", FrontmatterValue::Number(150.0)),
        ("true", FrontmatterValue::Bool(true)),
        ("False", FrontmatterValue::Bool(false)),
        ("FALSE", FrontmatterValue::Bool(false)),
        ("", FrontmatterValue::Null),
        ("~", FrontmatterValue::Null),
        ("Null", FrontmatterValue::Null),
        ("NULL", FrontmatterValue::Null),
        ("yes", FrontmatterValue::String("yes".into())),
        ("1e", FrontmatterValue::String("1e".into())),
        ("0O17", FrontmatterValue::String("0O17".into())),
    ] {
        let parsed = parse_frontmatter(&format!("---\nn: {yaml}\n---")).unwrap();
        assert_eq!(
            parsed.frontmatter,
            FrontmatterValue::Mapping(vec![("n".into(), expected)]),
            "{yaml}"
        );
    }
}

/// Expected single-description metadata with the unmodified fixture body.
fn description(text: &str) -> ParsedFrontmatter {
    ParsedFrontmatter {
        frontmatter: FrontmatterValue::Mapping(vec![(
            "description".into(),
            FrontmatterValue::String(text.into()),
        )]),
        body: "Body".into(),
    }
}

/// Adjacent escapes span supplementary boundaries without altering surrounding text.
#[test]
fn paired_escapes_decode_scalar_boundaries() {
    for (input, expected) in [
        (
            "---\ndescription: \"\\uD83C\\uDF89\"\n---\nBody",
            Ok(description("\u{1f389}")),
        ),
        (
            "---\ndescription: \"\\uD800\\uDC00\"\n---\nBody",
            Ok(description("\u{10000}")),
        ),
        (
            "---\ndescription: \"\\uDBFF\\uDFFF\"\n---\nBody",
            Ok(description("\u{10ffff}")),
        ),
        (
            "---\ndescription: \"\\uDBFF\\uDC00\"\n---\nBody",
            Ok(description("\u{10fc00}")),
        ),
        (
            "---\ndescription: \"\\uD800\\uDFFF\"\n---\nBody",
            Ok(description("\u{103ff}")),
        ),
        (
            "---\ndescription: \"pre\\ud83C\\uDf89post\"\n---\nBody",
            Ok(description("pre\u{1f389}post")),
        ),
        (
            "---\ndescription: \"\\uD83C\\uDF89\\uD834\\uDD1E\"\n---\nBody",
            Ok(description("\u{1f389}\u{1d11e}")),
        ),
        (
            "---\n\"\\uD83C\\uDF89\"\n---\nBody",
            Ok(ParsedFrontmatter {
                frontmatter: FrontmatterValue::String("\u{1f389}".into()),
                body: "Body".into(),
            }),
        ),
    ] {
        assert_eq!(parse_frontmatter(input), expected, "{input}");
    }
}

/// Two complete escapes advance markers to subsequent tokens and errors.
#[test]
fn paired_escapes_preserve_scanner_position() {
    for (input, expected) in [
        ("---\ndescription: \"\\uD83C\\uDF89\"\nafter: retained\n---\nBody", Ok(ParsedFrontmatter { frontmatter: FrontmatterValue::Mapping(vec![("description".into(), FrontmatterValue::String("\u{1f389}".into())), ("after".into(), FrontmatterValue::String("retained".into()))]), body: "Body".into() })),
        ("---\n[\"\\uD83C\\uDF89\", next, {key: \"\\uD834\\uDD1E\"}]\n---\nBody", Ok(ParsedFrontmatter { frontmatter: FrontmatterValue::Sequence(vec![FrontmatterValue::String("\u{1f389}".into()), FrontmatterValue::String("next".into()), FrontmatterValue::Mapping(vec![("key".into(), FrontmatterValue::String("\u{1d11e}".into()))])]), body: "Body".into() })),
        ("---\ndescription: \"\\uD83C\\uDF89\\n\\u0041\"\n---\nBody", Ok(description("\u{1f389}\nA"))),
        ("---\ndescription: \"\\uD83C\\uDF89\"\nafter: \"\\q\"\n---\nBody", Err(FrontmatterError { message: "while parsing a quoted scalar, found unknown escape character at byte 35 line 2 column 8".into(), line: Some(2), column: Some(8) })),
        ("---\ndescription: \"\\uD83C\\uDF89\" invalid\n---\nBody", Err(FrontmatterError { message: "invalid trailing content after double-quoted scalar at byte 28 line 1 column 29".into(), line: Some(1), column: Some(29) })),
    ] {
        assert_eq!(parse_frontmatter(input), expected, "{input}");
    }
}

/// Decoded keys retain equality, insertion order and collection-key discard behavior.
#[test]
fn paired_escapes_preserve_mapping_identity() {
    for (input, expected) in [
        (
            "---\nextra: {\"\\uD83C\\uDF89\": [\"\\uD834\\uDD1E\", \"literal\"]}\n---\nBody",
            Ok(ParsedFrontmatter {
                frontmatter: FrontmatterValue::Mapping(vec![(
                    "extra".into(),
                    FrontmatterValue::Mapping(vec![(
                        "\u{1f389}".into(),
                        FrontmatterValue::Sequence(vec![
                            FrontmatterValue::String("\u{1d11e}".into()),
                            FrontmatterValue::String("literal".into()),
                        ]),
                    )]),
                )]),
                body: "Body".into(),
            }),
        ),
        (
            "---\n\"\\uD83C\\uDF89\": first\n\"\\\\uD83C\\\\uDF89\": second\n\"other\": third\n---\nBody",
            Ok(ParsedFrontmatter {
                frontmatter: FrontmatterValue::Mapping(vec![
                    ("\u{1f389}".into(), FrontmatterValue::String("first".into())),
                    (
                        "\\uD83C\\uDF89".into(),
                        FrontmatterValue::String("second".into()),
                    ),
                    ("other".into(), FrontmatterValue::String("third".into())),
                ]),
                body: "Body".into(),
            }),
        ),
        (
            "---\n\"\\uD83C\\uDF89\": first\n\"\u{1f389}\": second\n---\nBody",
            Err(FrontmatterError {
                message: "Duplicated key in mapping".into(),
                line: None,
                column: None,
            }),
        ),
        (
            "---\n\"\u{1f389}\": first\n\"\\uD83C\\uDF89\": second\n---\nBody",
            Err(FrontmatterError {
                message: "Duplicated key in mapping".into(),
                line: None,
                column: None,
            }),
        ),
        (
            "---\n? [\"\\uD83C\\uDF89\"]\n: dropped\ndescription: retained\n---\nBody",
            Ok(description("retained")),
        ),
    ] {
        assert_eq!(parse_frontmatter(input), expected, "{input}");
    }
}

/// Explicit scalar tags and completed aliases retain the decoded character.
#[test]
fn paired_escapes_resolve_tags_and_aliases() {
    for (input, expected) in [
        (
            "---\ndescription: &a \"\\uD83C\\uDF89\"\nextra: *a\n---\nBody",
            Ok(ParsedFrontmatter {
                frontmatter: FrontmatterValue::Mapping(vec![
                    (
                        "description".into(),
                        FrontmatterValue::String("\u{1f389}".into()),
                    ),
                    ("extra".into(), FrontmatterValue::String("\u{1f389}".into())),
                ]),
                body: "Body".into(),
            }),
        ),
        (
            "---\ndescription: !!str \"\\uD83C\\uDF89\"\n---\nBody",
            Ok(description("\u{1f389}")),
        ),
        (
            "---\ndescription: !!int \"\\uD83C\\uDF89\"\n---\nBody",
            Ok(description("\u{1f389}")),
        ),
    ] {
        assert_eq!(parse_frontmatter(input), expected, "{input}");
    }
}

/// Only double-quoted scalar escape dispatch interprets paired backslashes.
#[test]
fn escape_scanning_respects_quoting_and_literal_text() {
    for (input, expected) in [
        (
            "---\ndescription: '\\uD83C\\uDF89'\n---\nBody",
            Ok(description("\\uD83C\\uDF89")),
        ),
        (
            "---\ndescription: \\uD83C\\uDF89\n---\nBody",
            Ok(description("\\uD83C\\uDF89")),
        ),
        (
            "---\ndescription: |\n  \\uD83C\\uDF89\n---\nBody",
            Ok(description("\\uD83C\\uDF89\n")),
        ),
        (
            "---\ndescription: >\n  \\uD83C\\uDF89\n---\nBody",
            Ok(description("\\uD83C\\uDF89\n")),
        ),
        (
            "---\ndescription: ok # \"\\uD800\"\n---\nBody",
            Ok(description("ok")),
        ),
        (
            "---\ndescription: \"\\\\uD83C\\\\uDF89\"\n---\nBody",
            Ok(description("\\uD83C\\uDF89")),
        ),
        (
            "---\ndescription: \"\\\"\\uD83C\\uDF89\"\n---\nBody",
            Ok(description("\"\u{1f389}")),
        ),
    ] {
        assert_eq!(parse_frontmatter(input), expected, "{input}");
    }
}

/// Valid short, BMP and eight-digit escapes keep their original scalar values.
#[test]
fn ordinary_scalar_escapes_keep_their_values() {
    for (input, expected) in [
        (
            "---\ndescription: \"\\uD7FF\\uE000\\uFFFF\\u0000\"\n---\nBody",
            Ok(description("\u{d7ff}\u{e000}\u{ffff}\0")),
        ),
        (
            "---\ndescription: \"\\U0001F389\"\n---\nBody",
            Ok(description("\u{1f389}")),
        ),
        (
            "---\ndescription: \"\\U00010000\\U0010FFFF\"\n---\nBody",
            Ok(description("\u{10000}\u{10ffff}")),
        ),
        (
            "---\ndescription: \"\\x00\\x41\\xFF\"\n---\nBody",
            Ok(description("\0A\u{ff}")),
        ),
        (
            "---\ndescription: \"\\0\\a\\b\\t\\n\\v\\f\\r\\e\\ \\\"\\/\\\\\\N\\_\\L\\P\"\n---\nBody",
            Ok(ParsedFrontmatter {
                frontmatter: FrontmatterValue::Mapping(vec![(
                    "description".into(),
                    FrontmatterValue::String(
                        "\0\u{7}\u{8}\t\n\u{b}\u{c}\r\u{1b} \"/\\\u{85}\u{a0}\u{2028}\u{2029}"
                            .into(),
                    ),
                )]),
                body: "Body".into(),
            }),
        ),
    ] {
        assert_eq!(parse_frontmatter(input), expected, "{input}");
    }
}

/// Unpaired and separated surrogate escapes keep exact native diagnostics.
#[test]
fn invalid_surrogates_keep_native_errors() {
    for (input, expected) in [
        ("---\ndescription: \"\\uD800\"\n---\nBody", Err(FrontmatterError { message: "while parsing a quoted scalar, found invalid Unicode character escape code at byte 13 line 1 column 14".into(), line: Some(1), column: Some(14) })),
        ("---\ndescription: \"\\uDBFF\"\n---\nBody", Err(FrontmatterError { message: "while parsing a quoted scalar, found invalid Unicode character escape code at byte 13 line 1 column 14".into(), line: Some(1), column: Some(14) })),
        ("---\ndescription: \"\\uDC00\"\n---\nBody", Err(FrontmatterError { message: "while parsing a quoted scalar, found invalid Unicode character escape code at byte 13 line 1 column 14".into(), line: Some(1), column: Some(14) })),
        ("---\ndescription: \"\\uDFFF\"\n---\nBody", Err(FrontmatterError { message: "while parsing a quoted scalar, found invalid Unicode character escape code at byte 13 line 1 column 14".into(), line: Some(1), column: Some(14) })),
        ("---\ndescription: \"\\uDF89\\uD83C\"\n---\nBody", Err(FrontmatterError { message: "while parsing a quoted scalar, found invalid Unicode character escape code at byte 13 line 1 column 14".into(), line: Some(1), column: Some(14) })),
        ("---\ndescription: \"\\uD800\\uDBFF\"\n---\nBody", Err(FrontmatterError { message: "while parsing a quoted scalar, found invalid Unicode character escape code at byte 13 line 1 column 14".into(), line: Some(1), column: Some(14) })),
        ("---\ndescription: \"\\uD800\\uDBFFsuffix\"\n---\nBody", Err(FrontmatterError { message: "while parsing a quoted scalar, found invalid Unicode character escape code at byte 13 line 1 column 14".into(), line: Some(1), column: Some(14) })),
        ("---\ndescription: \"\\uD800\\uE000\"\n---\nBody", Err(FrontmatterError { message: "while parsing a quoted scalar, found invalid Unicode character escape code at byte 13 line 1 column 14".into(), line: Some(1), column: Some(14) })),
        ("---\ndescription: \"\\uD800\\u0041\"\n---\nBody", Err(FrontmatterError { message: "while parsing a quoted scalar, found invalid Unicode character escape code at byte 13 line 1 column 14".into(), line: Some(1), column: Some(14) })),
        ("---\ndescription: \"\\uD83C \\uDF89\"\n---\nBody", Err(FrontmatterError { message: "while parsing a quoted scalar, found invalid Unicode character escape code at byte 13 line 1 column 14".into(), line: Some(1), column: Some(14) })),
        ("---\ndescription: \"\\uD83C\n  \\uDF89\"\n---\nBody", Err(FrontmatterError { message: "while parsing a quoted scalar, found invalid Unicode character escape code at byte 13 line 1 column 14".into(), line: Some(1), column: Some(14) })),
        ("---\ndescription: \"\\uD83C\\\n  \\uDF89\"\n---\nBody", Err(FrontmatterError { message: "while parsing a quoted scalar, found invalid Unicode character escape code at byte 13 line 1 column 14".into(), line: Some(1), column: Some(14) })),
        ("---\ndescription: \"\\uD83C\\\\uDF89\"\n---\nBody", Err(FrontmatterError { message: "while parsing a quoted scalar, found invalid Unicode character escape code at byte 13 line 1 column 14".into(), line: Some(1), column: Some(14) })),
        ("---\ndescription: \"\\uD83C\\U0000DF89\"\n---\nBody", Err(FrontmatterError { message: "while parsing a quoted scalar, found invalid Unicode character escape code at byte 13 line 1 column 14".into(), line: Some(1), column: Some(14) })),
        ("---\ndescription: \"\\U0000D83C\\uDF89\"\n---\nBody", Err(FrontmatterError { message: "while parsing a quoted scalar, found invalid Unicode character escape code at byte 13 line 1 column 14".into(), line: Some(1), column: Some(14) })),
        ("---\ndescription: \"\\uD83C\\uDF89\\uD800\"\n---\nBody", Err(FrontmatterError { message: "while parsing a quoted scalar, found invalid Unicode character escape code at byte 13 line 1 column 14".into(), line: Some(1), column: Some(14) })),
        ("---\ndescription: ok\nextra: \"\\uD800\"\n---\nBody", Err(FrontmatterError { message: "while parsing a quoted scalar, found invalid Unicode character escape code at byte 23 line 2 column 8".into(), line: Some(2), column: Some(8) })),
        ("---\ndescription: ok\n\"\\uD800\": ignored\n---\nBody", Err(FrontmatterError { message: "while parsing a quoted scalar, found invalid Unicode character escape code at byte 16 line 2 column 1".into(), line: Some(2), column: Some(1) })),
        ("---\n? [\"\\uD800\"]\n: discarded\ndescription: kept\n---\nBody", Err(FrontmatterError { message: "while parsing a quoted scalar, found invalid Unicode character escape code at byte 3 line 1 column 4".into(), line: Some(1), column: Some(4) })),
    ] {
        assert_eq!(parse_frontmatter(input), expected, "{input}");
    }
}

/// Malformed numeric escapes preserve native error precedence and coordinates.
#[test]
fn malformed_escapes_keep_native_errors() {
    for (input, expected) in [
        ("---\ndescription: \"\\uGGGG\"\n---\nBody", Err(FrontmatterError { message: "while parsing a quoted scalar, did not find expected hexadecimal number at byte 13 line 1 column 14".into(), line: Some(1), column: Some(14) })),
        ("---\ndescription: \"\\uD83C\\uGGGG\"\n---\nBody", Err(FrontmatterError { message: "while parsing a quoted scalar, found invalid Unicode character escape code at byte 13 line 1 column 14".into(), line: Some(1), column: Some(14) })),
        ("---\ndescription: \"\\uD83\"\n---\nBody", Err(FrontmatterError { message: "while parsing a quoted scalar, did not find expected hexadecimal number at byte 13 line 1 column 14".into(), line: Some(1), column: Some(14) })),
        ("---\ndescription: \"\\uD83C\\uDF8\"\n---\nBody", Err(FrontmatterError { message: "while parsing a quoted scalar, found invalid Unicode character escape code at byte 13 line 1 column 14".into(), line: Some(1), column: Some(14) })),
        ("---\ndescription: \"\\uD83C\\u\"\n---\nBody", Err(FrontmatterError { message: "while parsing a quoted scalar, found invalid Unicode character escape code at byte 13 line 1 column 14".into(), line: Some(1), column: Some(14) })),
        ("---\ndescription: \"\\U00110000\"\n---\nBody", Err(FrontmatterError { message: "while parsing a quoted scalar, found invalid Unicode character escape code at byte 13 line 1 column 14".into(), line: Some(1), column: Some(14) })),
        ("---\ndescription: \"\\q\"\n---\nBody", Err(FrontmatterError { message: "while parsing a quoted scalar, found unknown escape character at byte 13 line 1 column 14".into(), line: Some(1), column: Some(14) })),
    ] {
        assert_eq!(parse_frontmatter(input), expected, "{input}");
    }
}

/// Duplicate keys, incomplete collections and cyclic aliases keep native errors.
#[test]
fn frontmatter_structure_errors_keep_native_errors() {
    for (input, expected) in [
        (
            "---\ndescription: a\ndescription: b\n---\nBody",
            Err(FrontmatterError {
                message: "Duplicated key in mapping".into(),
                line: None,
                column: None,
            }),
        ),
        (
            "---\nfoo: [bar\n---\nBody",
            Err(FrontmatterError {
                message:
                    "while parsing a flow sequence, expected ',' or ']' at byte 9 line 2 column 1"
                        .into(),
                line: Some(2),
                column: Some(1),
            }),
        ),
        (
            "---\ndescription: ok\nextra: &a {self: *a}\n---\nBody",
            Err(FrontmatterError {
                message: "Unresolved YAML alias".into(),
                line: None,
                column: None,
            }),
        ),
    ] {
        assert_eq!(parse_frontmatter(input), expected, "{input}");
    }
}

/// Body-only extraction still validates metadata and uses existing whitespace rules.
#[test]
fn strip_frontmatter_validates_pair_escapes() {
    for (input, body) in [
        (
            "---\ndescription: \"\\uD83C\\uDF89\"\n---\n\u{feff} Body \u{feff}",
            "Body",
        ),
        (
            "---\ndescription: \"\\uD83C\\uDF89\"\n---\n\u{85}Body\u{85}",
            "\u{85}Body\u{85}",
        ),
        (
            "---\r\ndescription: \"\\uD83C\\uDF89\"\r\n---\r\nBody\r\nnext",
            "Body\nnext",
        ),
        (
            "description: \"\\uD800\"\nBody",
            "description: \"\\uD800\"\nBody",
        ),
        (
            "---\ndescription: \"\\uD800\"\nBody",
            "---\ndescription: \"\\uD800\"\nBody",
        ),
    ] {
        assert_eq!(strip_frontmatter(input).unwrap(), body, "{input}");
    }
    assert_eq!(strip_frontmatter("---\ndescription: \"\\uD800\"\n---\nBody").unwrap_err(), FrontmatterError { message: "while parsing a quoted scalar, found invalid Unicode character escape code at byte 13 line 1 column 14".into(), line: Some(1), column: Some(14) });
}
