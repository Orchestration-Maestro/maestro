//! Office math rendered as `MathML`.
use super::check_preview;
use crate::docx_preview::{Options, parse_async};
use serde_json::Value;
use std::path::Path;

#[test]
fn equations_render_as_mathml_structures() {
    check_preview("math-structures");
}

#[test]
fn equation_paragraphs_and_styled_runs_keep_classes_and_declarations() {
    check_preview("math-paragraph-and-runs");
}

/// An equation element the reference cannot read renders with the documented default instead
/// of failing the document.
#[test]
fn malformed_equations_render_without_failing_the_document() {
    for (name, tag, expected_text) in [
        ("math-nary-single-script", "mover", "∫"),
        ("math-nary-without-base", "munderover", "∫"),
        ("math-presubsup-single-script", "msubsup", "l"),
        ("math-groupchr-without-properties", "munder", "g"),
        ("math-bar-without-properties", "mrow", "o"),
        ("math-delimiter-without-properties", "mrow", "p"),
        ("math-radical-without-degree", "msqrt", "w"),
    ] {
        let body = render_math(name);

        let node = find(&body, tag).unwrap_or_else(|| panic!("{name} has no {tag}"));
        assert!(text(node).contains(expected_text), "{name} {tag}");
    }
}

/// Render one recorded fixture that the reference fails on, and observe its container.
fn render_math(name: &str) -> Vec<Value> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/docx_preview/fixtures/docx")
        .join(format!("{name}.docx"));
    let package = std::fs::read(path).unwrap();
    let options = Options::default();
    let preview = parse_async(&package, options.clone()).unwrap();
    let rendering = preview.render(&options);
    rendering.body.iter().map(super::observe_preview).collect()
}

/// The first element with a tag.
fn find<'v>(nodes: &'v [Value], tag: &str) -> Option<&'v Value> {
    for node in nodes {
        if node["tag"] == tag {
            return Some(node);
        }
        if let Some(children) = node["children"].as_array()
            && let Some(found) = find(children, tag)
        {
            return Some(found);
        }
    }
    None
}

/// The text an element holds, its children's text included.
fn text(node: &Value) -> String {
    if let Some(text) = node["text"].as_str() {
        return text.to_owned();
    }
    node["children"]
        .as_array()
        .map(|children| children.iter().map(text).collect())
        .unwrap_or_default()
}
