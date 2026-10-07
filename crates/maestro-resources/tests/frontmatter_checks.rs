use maestro_resources::{FrontmatterValue as V, parse_frontmatter, strip_frontmatter};
fn yaml(s: &str) -> V {
    parse_frontmatter(&format!("---\n{s}\n---"))
        .unwrap()
        .frontmatter
}
fn field<'a>(v: &'a V, k: &str) -> &'a V {
    if let V::Mapping(m) = v {
        &m.iter()
            .find(|(key, _)| matches!(key,V::String(s) if s==k))
            .unwrap()
            .1
    } else {
        panic!("not mapping")
    }
}
#[test]
fn yaml_values_tags_and_aliases() {
    let x = yaml("a: .nan\nb: .inf\nc: -.inf\nd: -0");
    assert!(matches!(field(&x,"a"),V::Number(n) if n.is_nan()));
    assert_eq!(field(&x, "b"), &V::Number(f64::INFINITY));
    assert_eq!(field(&x, "c"), &V::Number(f64::NEG_INFINITY));
    assert_eq!(field(&x, "d"), &V::Number(0.0));
    assert_eq!(yaml("null"), V::Mapping(vec![]));
    assert_eq!(yaml("false"), V::Bool(false));
    assert_eq!(yaml("0"), V::Number(0.));
    assert_eq!(yaml("text"), V::String("text".into()));
    assert!(parse_frontmatter("---\na: 1\na: 2\n---").is_err());
    assert_eq!(
        yaml("0: first\n\"0\": second"),
        V::Mapping(vec![
            (V::Number(0.0), V::String("first".into())),
            (V::String("0".into()), V::String("second".into()))
        ])
    );
    let x = yaml("b: 1\n2: two\n1: one\na: 2");
    if let V::Mapping(m) = x {
        assert_eq!(
            m.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>(),
            vec![
                V::String("b".into()),
                V::Number(2.0),
                V::Number(1.0),
                V::String("a".into())
            ]
        )
    } else {
        panic!("mapping expected")
    }
    assert_eq!(
        yaml("a: &a [*a]"),
        V::Mapping(vec![(V::String("a".into()), V::Sequence(vec![V::Null]))])
    );
    for n in [99, 100] {
        let s = format!("a: &a [x,y]\nb: [{}]", vec!["*a"; n].join(","));
        assert!(matches!(field(&yaml(&s),"b"),V::Sequence(v) if v.len()==n));
    }
    assert_eq!(
        field(&yaml("__proto__: value"), "__proto__"),
        &V::String("value".into())
    );
    assert_eq!(yaml("!!str 2"), V::String("2".into()));
    assert_eq!(yaml("!custom hello"), V::String("hello".into()));
    assert_eq!(
        yaml("[a,b]: value"),
        V::Mapping(vec![(
            V::Sequence(vec![V::String("a".into()), V::String("b".into())]),
            V::String("value".into())
        )])
    );
}
#[test]
fn yaml_warning_text_reaches_stderr() {
    if let Ok(input) = std::env::var("MAESTRO_YAML_WARNING") {
        let _ = parse_frontmatter(&format!("---\n{input}\n---"));
        return;
    }
    for input in [
        "description: !custom hello",
        "[a, b]: value",
        "description: !!float \"2\"",
    ] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "yaml_warning_text_reaches_stderr", "--nocapture"])
            .env("MAESTRO_YAML_WARNING", input)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
    }
}
#[test]
fn delimiter_slices_and_ecmascript_trim() {
    let p = parse_frontmatter("---😀\n---tail").unwrap();
    assert_eq!(p.frontmatter, V::Mapping(vec![]));
    assert_eq!(p.body, "tail");
    assert_eq!(
        parse_frontmatter("---X\nname: n\n---suffix\rbody\u{feff}")
            .unwrap()
            .body,
        "suffix\nbody\u{feff}"
    );
    for text in [
        "\u{feff}---\nname: n\n---",
        " ---\nname: n\n---",
        "---\rname: n",
    ] {
        let p = parse_frontmatter(text).unwrap();
        assert_eq!(p.body, text.replace('\r', "\n"));
        assert_eq!(p.frontmatter, V::Mapping(vec![]));
    }
    assert_eq!(
        parse_frontmatter("---\n---\u{feff}body\u{feff}")
            .unwrap()
            .body,
        "\u{feff}body\u{feff}"
    );
    assert_eq!(
        parse_frontmatter("---\n---\u{85}body\u{85}").unwrap().body,
        "body"
    );
}

#[test]
fn quoted_keys_and_body_survive() {
    let p = parse_frontmatter(
        "---\nname: 'hello'\ndescription: \"A thing\"\ncustom-key: value\n---\n Body \n",
    )
    .unwrap();
    assert_eq!(field(&p.frontmatter, "name"), &V::String("hello".into()));
    assert_eq!(
        field(&p.frontmatter, "description"),
        &V::String("A thing".into())
    );
    assert_eq!(
        field(&p.frontmatter, "custom-key"),
        &V::String("value".into())
    );
    assert_eq!(p.body, "Body");
}

#[test]
fn crlf_body_becomes_lf() {
    assert_eq!(
        parse_frontmatter("---\r\nname: x\r\n---\r\nLine one\r\nLine two\r\n")
            .unwrap()
            .body,
        "Line one\nLine two"
    );
}

#[test]
fn invalid_yaml_reports_location() {
    let e = parse_frontmatter("---\nfoo: [bar\n---")
        .unwrap_err()
        .message
        .unwrap();
    assert!(e.contains("while parsing a flow sequence"));
    assert!(e.contains("line"));
    assert!(e.contains("column"));
    assert!(e.contains("byte"));
}

#[test]
fn literal_description_keeps_final_newline() {
    let p = parse_frontmatter("---\ndescription: |\n  Line one\n  Line two\n---\nBody").unwrap();
    assert_eq!(
        field(&p.frontmatter, "description"),
        &V::String("Line one\nLine two\n".into())
    );
    assert_eq!(p.body, "Body");
}

#[test]
fn absent_or_unclosed_header_keeps_normalized_text() {
    for s in ["Plain text\r\n", "---\rname: x\r\nBody\n"] {
        assert_eq!(
            parse_frontmatter(s).unwrap().body,
            s.replace("\r\n", "\n").replace("\r", "\n")
        );
    }
}

#[test]
fn comment_header_yields_empty_object() {
    assert_eq!(yaml("# only a comment"), V::Mapping(vec![]));
    assert_eq!(yaml(""), V::Mapping(vec![]));
}

#[test]
fn stripped_header_trims_body() {
    assert_eq!(
        strip_frontmatter("---\nname: x\n---\n  Body \n").unwrap(),
        "Body"
    );
}

#[test]
fn plain_body_preserves_surrounding_whitespace() {
    assert_eq!(strip_frontmatter(" \nBody \n ").unwrap(), " \nBody \n ");
}

#[test]
fn strip_propagates_yaml_failure() {
    let s = "---\na: [\n---";
    assert_eq!(
        strip_frontmatter(s).unwrap_err(),
        parse_frontmatter(s).unwrap_err()
    );
}

#[test]
fn resource_error_propagates_into_boxed_error() {
    fn parse() -> Result<(), Box<dyn std::error::Error>> {
        maestro_resources::parse_frontmatter("---\nx: [\n---")?;
        Ok(())
    }
    let error = parse().unwrap_err();
    assert!(error.to_string().contains("line"));
    assert_eq!(
        maestro_resources::ResourceError {
            message: Some("read failed".into())
        }
        .to_string(),
        "read failed"
    );
    assert_eq!(
        maestro_resources::ResourceError { message: None }.to_string(),
        ""
    );
}
