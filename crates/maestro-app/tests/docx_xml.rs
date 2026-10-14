//! Borrowed Word XML queries.
#![cfg(test)]
mod docx_support;
use docx_support::cases;
use maestro_app::browser::docx_preview::{
    DocxError, LengthUsage, XmlElement, XmlParser, parse_xml_string,
};
use serde::Deserialize;
use serde_json::{Value, json};

/// XML or numeric-attribute arguments.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// XML source.
    xml: Option<String>,
    /// Declaration policy.
    trim: Option<bool>,
    /// Numeric source spelling.
    value: Option<String>,
}

/// Project an expected mixed tree onto the borrowed query surface.
fn expected_tree(tree: &Value) -> Value {
    let children = tree["children"].as_array().unwrap();
    let elements: Vec<_> = children
        .iter()
        .filter(|c| c.is_object())
        .map(expected_tree)
        .collect();
    let text: String = children
        .iter()
        .map(|c| {
            c.as_str().map_or_else(
                || expected_tree(c)["text"].as_str().unwrap().to_owned(),
                str::to_owned,
            )
        })
        .collect();
    let mut tree = tree.clone();
    tree["children"] = json!(elements);
    tree["text"] = json!(text);
    tree
}

/// Observe element and attribute order without exposing parser handles.
fn observed_tree<'a>(parser: &XmlParser<'a>, element: XmlElement<'_, 'a>) -> Value {
    json!({
        "local_name": element.local_name(), "namespace": element.namespace(),
        "attributes": parser.attrs(element).map(|a| json!({
            "local_name": a.local_name, "namespace": a.namespace, "value": a.value
        })).collect::<Vec<_>>(),
        "children": parser.elements(element, None).map(|c| observed_tree(parser, c)).collect::<Vec<_>>(),
        "text": element.text_content()
    })
}

/// Replay XML admission and query observations.
fn check(name: &str) {
    for case in cases::<Input>(include_str!("fixtures/docx/xml.json"), name) {
        assert!(case.input.value.is_none());
        let parser = parse_xml_string(case.input.xml.as_deref().unwrap(), case.input.trim.unwrap());
        if case.expected.get("error").is_some() {
            assert!(matches!(parser, Err(DocxError::Xml { path: None, .. })));
            assert_eq!(case.expected, json!({"error": "Xml"}));
            continue;
        }
        let parser = parser.unwrap();
        let root = parser.root();
        let mut observed = json!({
            "tree": observed_tree(&parser, root),
            "element_names": parser.elements(root, None).map(|c| c.local_name()).collect::<Vec<_>>(),
            "selected_x": parser.elements(root, Some("x")).map(|c| parser.attr(c, "n")).collect::<Vec<_>>(),
            "first_v": parser.attr(root, "v"), "missing": parser.element_attr(root, "missing", "n"),
            "text": root.text_content()
        });
        if let Some(first) = parser.element(root, "x") {
            observed["first_x"] = json!(parser.attr(first, "n"));
        }
        let mut expected = case.expected;
        expected["tree"] = expected_tree(&expected["tree"]);
        assert_eq!(observed, expected);
    }
}

#[test]
fn xml_children_and_attributes_keep_local_names_and_order() {
    check("xml_children_and_attributes_keep_local_names_and_order");
}

#[test]
fn xml_entities_cdata_and_whitespace_survive() {
    check("xml_entities_cdata_and_whitespace_survive");
}

#[test]
fn xml_namespaces_are_scoped_not_inferred_from_prefixes() {
    check("xml_namespaces_are_scoped_not_inferred_from_prefixes");
}

#[test]
fn xml_declarations_and_bom_follow_the_option() {
    check("xml_declarations_and_bom_follow_the_option");
}

#[test]
fn xml_declaration_trimming_preserves_content_between_instructions() {
    check("xml_declaration_trimming_preserves_content_between_instructions");
}

#[test]
fn xml_parsererror_elements_are_data() {
    check("xml_parsererror_elements_are_data");
}

#[test]
fn xml_invalid_documents_fail_without_partial_success() {
    check("xml_invalid_documents_fail_without_partial_success");
}

#[test]
fn xml_doctype_is_rejected_at_the_package_boundary() {
    check("xml_doctype_is_rejected_at_the_package_boundary");
}

/// Compare nullable numbers, including nonfinite and signed-zero outcomes.
fn assert_number(actual: Option<f64>, expected: &Value) {
    if expected.is_null() {
        assert!(actual.is_none());
        return;
    }
    let actual = actual.unwrap();
    match expected.get("special").and_then(Value::as_str) {
        Some("nan") => assert!(actual.is_nan()),
        Some("negative_zero") => assert_eq!(actual.to_bits(), (-0.0f64).to_bits()),
        Some("negative_infinity") => assert_eq!(actual.to_bits(), f64::NEG_INFINITY.to_bits()),
        Some("positive_infinity") => assert_eq!(actual.to_bits(), f64::INFINITY.to_bits()),
        None => assert_eq!(actual.to_bits(), expected.as_f64().unwrap().to_bits()),
        _ => panic!("unknown numeric observation"),
    }
}

#[test]
fn xml_numeric_attributes_keep_empty_defaults_and_prefix_rules() {
    for case in cases::<Input>(
        include_str!("fixtures/docx/xml.json"),
        "xml_numeric_attributes_keep_empty_defaults_and_prefix_rules",
    ) {
        assert!(case.input.xml.is_none() && case.input.trim.is_none());
        let source = case
            .input
            .value
            .map_or_else(|| "<r/>".into(), |v| format!("<r v=\"{v}\"/>"));
        let parser = parse_xml_string(&source, false).unwrap();
        let root = parser.root();
        assert_number(
            parser.int_attr(root, "v", Some(17.0)),
            &case.expected["int"],
        );
        assert_number(
            parser.hex_attr(root, "v", Some(17.0)),
            &case.expected["hex"],
        );
        assert_number(
            parser.float_attr(root, "v", Some(17.0)),
            &case.expected["float"],
        );
        let mut remainder = case.expected;
        for key in ["int", "hex", "float"] {
            remainder.as_object_mut().unwrap().shift_remove(key);
        }
        assert_eq!(
            json!({"bool": parser.bool_attr(root, "v", Some(true)), "length": parser.length_attr(root, "v", LengthUsage::Dxa)}),
            remainder
        );
    }
}
