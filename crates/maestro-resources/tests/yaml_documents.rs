//! Raw YAML values use the metadata parser without header extraction.
#![cfg(test)]
use maestro_resources::frontmatter::{FrontmatterValue as V, parse_frontmatter, parse_yaml};

#[test]
fn raw_yaml_preserves_document_values() {
    for (source, expected) in [
        ("", V::Null),
        ("null", V::Null),
        ("true", V::Bool(true)),
        ("42", V::Number(42.0)),
        ("plain text", V::String("plain text".into())),
        ("[null, false]", V::Sequence(vec![V::Null, V::Bool(false)])),
        (
            "z: &a [one]\na: *a",
            V::Mapping(vec![
                ("z".into(), V::Sequence(vec![V::String("one".into())])),
                ("a".into(), V::Sequence(vec![V::String("one".into())])),
            ]),
        ),
        (
            "text: |\n  ---\n  ...\n",
            V::Mapping(vec![("text".into(), V::String("---\n...\n".into()))]),
        ),
        (
            "---\r\n[1, 2]\r\n...",
            V::Sequence(vec![V::Number(1.0), V::Number(2.0)]),
        ),
    ] {
        assert_eq!(parse_yaml(source).unwrap(), expected, "{source}");
    }
}

#[test]
fn raw_yaml_preserves_exceptional_numbers() {
    assert!(matches!(parse_yaml(".nan").unwrap(), V::Number(n) if n.is_nan()));
    assert!(matches!(parse_yaml(".inf").unwrap(), V::Number(n) if n == f64::INFINITY));
    assert!(matches!(parse_yaml("-0.0").unwrap(), V::Number(n) if n.is_sign_negative()));
}

#[test]
fn raw_yaml_errors_keep_coordinates() {
    for source in ["a: [", "---\none\n---\ntwo"] {
        let error = parse_yaml(source).unwrap_err();
        assert!(error.line.is_some(), "{error}");
        assert!(error.column.is_some(), "{error}");
        assert!(error.to_string().contains("line"));
    }
}

#[test]
fn frontmatter_keeps_null_header_normalization() {
    assert_eq!(
        parse_frontmatter("null").unwrap().frontmatter,
        V::Mapping(vec![])
    );
    assert_eq!(
        parse_frontmatter("---\nnull\n---\nbody")
            .unwrap()
            .frontmatter,
        V::Mapping(vec![])
    );
    assert_eq!(
        parse_frontmatter("---\nnull\n---\nbody").unwrap().body,
        "body"
    );
}
