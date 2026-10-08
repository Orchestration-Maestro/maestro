use maestro_resources::{FrontmatterValue, parse_frontmatter, strip_frontmatter};

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
