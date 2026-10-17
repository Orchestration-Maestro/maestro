//! Owned Word element parsing.
#![cfg(test)]
use crate::body_tree;
use crate::docx_support;
use docx_support::cases;
use maestro_app::browser::docx_preview::{
    DocxError, OwnedXmlElement, parse_body_elements, parse_paragraph, parse_run, parse_xml_string,
};
use serde::Deserialize;
use serde_json::json;
use std::fmt::Write;

/// XML framing arguments from the controlled oracle.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Selected operation.
    operation: String,
    /// Complete XML input.
    xml: String,
}
/// Replay every case owned by one behavior.
fn check(name: &str) {
    for case in cases::<Input>(include_str!("fixtures/docx/body.json"), name) {
        let parser = parse_xml_string(&case.input.xml, true);
        if case.expected.get("error_kind").is_some() {
            assert!(matches!(parser, Err(DocxError::Xml { .. })));
            assert_eq!(case.expected, json!({"error_kind":"Xml"}));
            continue;
        }
        let parser = parser.unwrap();
        let input = OwnedXmlElement::new(parser.root());
        let tree = match case.input.operation.as_str() {
            "body" => parse_body_elements(&input),
            "paragraph" => parse_paragraph(&input),
            "run" => parse_run(&input),
            _ => panic!("unknown framing"),
        }
        .unwrap();
        let observed = if case.expected.get("foreign_slots").is_some() {
            let mut observed =
                json!({"foreign_slots":body_tree::foreign(&tree, &case.input.operation)});
            if case.expected.get("typed_markers").is_some() {
                use maestro_app::browser::docx_preview::OpenXmlElementData as Data;
                observed["typed_markers"] = json!(
                    tree.nodes
                        .iter()
                        .filter_map(|n| match &n.data {
                            Data::CommentRangeStart(c) =>
                                Some(json!({"type":n.dom_type().as_str(),"id":c.id})),
                            Data::CommentRangeEnd(c) =>
                                Some(json!({"type":n.dom_type().as_str(),"id":c.id})),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                );
            }
            observed
        } else {
            body_tree::graph(&tree)
        };
        assert_eq!(observed, case.expected);
    }
}
#[test]
fn body_children_keep_order_and_repeats() {
    check("body_children_keep_order_and_repeats");
}

#[test]
fn body_ignores_unknown_and_non_elements() {
    check("body_ignores_unknown_and_non_elements");
}

#[test]
fn body_content_controls_handle_empty_and_missing() {
    check("body_content_controls_handle_empty_and_missing");
}

#[test]
fn body_content_controls_flatten_recursively() {
    check("body_content_controls_flatten_recursively");
}

#[test]
fn content_controls_use_first_content_child() {
    check("content_controls_use_first_content_child");
}

#[test]
fn alternate_chunks_keep_nullable_ids() {
    check("alternate_chunks_keep_nullable_ids");
}

#[test]
fn paragraph_runs_keep_separate_identity() {
    check("paragraph_runs_keep_separate_identity");
}

#[test]
fn paragraph_ignores_unknown_and_non_elements() {
    check("paragraph_ignores_unknown_and_non_elements");
}

#[test]
fn paragraph_content_controls_keep_hidden_parents() {
    check("paragraph_content_controls_keep_hidden_parents");
}

#[test]
fn paragraph_content_controls_handle_absence() {
    check("paragraph_content_controls_handle_absence");
}

#[test]
fn change_wrappers_keep_kind_children_and_parents() {
    check("change_wrappers_keep_kind_children_and_parents");
}

#[test]
fn empty_change_wrappers_are_retained() {
    check("empty_change_wrappers_are_retained");
}

#[test]
fn nested_changes_and_controls_preserve_structure() {
    check("nested_changes_and_controls_preserve_structure");
}

#[test]
fn run_text_keeps_whitespace_entities_and_cdata() {
    check("run_text_keeps_whitespace_entities_and_cdata");
}

#[test]
fn run_empty_text_nodes_remain_distinct() {
    check("run_empty_text_nodes_remain_distinct");
}

#[test]
fn run_ignores_unknown_and_outside_text() {
    check("run_ignores_unknown_and_outside_text");
}

#[test]
fn breaks_apply_only_the_empty_type_default() {
    check("breaks_apply_only_the_empty_type_default");
}

#[test]
fn run_markers_and_references_keep_order() {
    check("run_markers_and_references_keep_order");
}

#[test]
fn note_references_keep_string_identity() {
    check("note_references_keep_string_identity");
}

#[test]
fn simple_fields_keep_instruction_and_flags() {
    check("simple_fields_keep_instruction_and_flags");
}

#[test]
fn field_flags_use_shared_boolean_tokens() {
    check("field_flags_use_shared_boolean_tokens");
}

#[test]
fn field_flags_are_selected_independently() {
    check("field_flags_are_selected_independently");
}

#[test]
fn complex_fields_keep_open_character_type() {
    check("complex_fields_keep_open_character_type");
}

#[test]
fn instruction_and_complex_fields_mark_the_run() {
    check("instruction_and_complex_fields_mark_the_run");
}

#[test]
fn symbols_keep_font_and_hex_text() {
    check("symbols_keep_font_and_hex_text");
}

/// Font inputs admitted by document XML.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FontInput {
    /// Nullable decoded font name.
    font: Option<String>,
}
#[test]
fn font_family_enclosure_uses_shared_whitespace() {
    use maestro_app::browser::docx_preview::OpenXmlElementData;
    for case in cases::<FontInput>(
        include_str!("fixtures/docx/fonts.json"),
        "font_family_enclosure_uses_shared_whitespace",
    ) {
        let attribute = case.input.font.map_or(String::new(), |font| {
            let escaped = font.chars().fold(String::new(), |mut result, c| {
                write!(result, "&#{};", u32::from(c)).unwrap();
                result
            });
            format!(" font=\"{escaped}\"")
        });
        let source = format!("<r><sym{attribute}/></r>");
        let xml = parse_xml_string(&source, false).unwrap();
        let tree = parse_run(&OwnedXmlElement::new(xml.root())).unwrap();
        let OpenXmlElementData::Symbol { font, .. } =
            &tree.nodes[tree.nodes[tree.roots[0]].base.children[0]].data
        else {
            panic!("symbol missing")
        };
        assert_eq!(json!(font), case.expected);
    }
}

#[test]
fn run_local_names_accept_namespace_aliases() {
    check("run_local_names_accept_namespace_aliases");
}

#[test]
fn body_and_paragraph_local_names_ignore_prefix() {
    check("body_and_paragraph_local_names_ignore_prefix");
}

#[test]
fn body_foreign_branches_remain_owned() {
    check("body_foreign_branches_remain_owned");
}

#[test]
fn paragraph_foreign_branches_remain_owned() {
    check("paragraph_foreign_branches_remain_owned");
}

#[test]
fn malformed_xml_fails_before_tree_publication() {
    check("malformed_xml_fails_before_tree_publication");
}

#[test]
fn paragraph_simple_fields_preserve_visible_result() {
    check("paragraph_simple_fields_preserve_visible_result");
}

#[test]
fn paragraph_simple_fields_preserve_nested_results() {
    check("paragraph_simple_fields_preserve_nested_results");
}

#[test]
fn owned_xml_slices_keep_source_and_namespaces() {
    let source = "\u{feff}<?xml version=\"1.0\"?><w:root xmlns:w=\"urn:w\">é<w:r><w:t>retained</w:t><w:rPr/></w:r></w:root>".to_owned();
    let (owned, range) = {
        let xml = parse_xml_string(&source, true).unwrap();
        let run = xml.element(xml.root(), "r").unwrap();
        (OwnedXmlElement::new(run), run.source_range())
    };
    assert_eq!(owned.source_range(), range);
    assert_eq!(
        &owned.source()[range],
        "<w:r><w:t>retained</w:t><w:rPr/></w:r>"
    );
    let tree = parse_run(&owned).unwrap();
    let root = &tree.nodes[tree.roots[0]];
    let maestro_app::browser::docx_preview::OpenXmlElementData::Run { properties, .. } = &root.data
    else {
        panic!("run missing")
    };
    assert!(properties.css_style.as_ref().unwrap().is_empty());
    assert_eq!(
        body_tree::graph(&tree),
        json!({"roots":[0],"nodes":[{"type":"run","parent":null,"children":[1]},{"type":"text","text":"retained"}]})
    );
}

#[test]
fn parsed_tree_survives_source_release() {
    use maestro_app::browser::docx_preview::OpenXmlElementData as Data;
    let tree = {
        let source =
            "<w:p xmlns:w=\"urn:w\"><w:pPr><w:pStyle val=\"outer\"/></w:pPr><w:r><w:rPr><w:rStyle val=\"inner\"/></w:rPr><w:t>kept</w:t><w:commentReference id=\"kept-id\"/></w:r><w:hyperlink id=\"link\"/></w:p>"
                .to_owned();
        let xml = parse_xml_string(&source, false).unwrap();
        let input = OwnedXmlElement::new(xml.root());
        parse_paragraph(&input).unwrap()
    };
    let paragraph = &tree.nodes[tree.roots[0]];
    let run_id = paragraph.base.children[0];
    let run = &tree.nodes[run_id];
    assert_eq!(run.base.parent, Some(tree.roots[0]));
    let Data::Text { text } = &tree.nodes[run.base.children[0]].data else {
        panic!("text missing")
    };
    assert_eq!(text, "kept");
    let Data::CommentReference(reference) = &tree.nodes[run.base.children[1]].data else {
        panic!("comment reference missing")
    };
    assert_eq!(reference.id.as_deref(), Some("kept-id"));
    let Data::Unparsed { element, .. } = &tree.nodes[paragraph.base.children[1]].data else {
        panic!("hyperlink missing")
    };
    let Data::Paragraph { properties } = &paragraph.data else {
        panic!("paragraph missing")
    };
    assert_eq!(
        properties.style_name.as_ref().unwrap().as_deref(),
        Some("outer")
    );
    let Data::Run { properties, .. } = &run.data else {
        panic!("run missing")
    };
    assert_eq!(
        properties.style_name.as_ref().unwrap().as_deref(),
        Some("inner")
    );
    assert_eq!(body_tree::raw(element)["local_name"], "hyperlink");
    assert_eq!(body_tree::raw(element)["namespace"], "urn:w");
}

#[test]
fn simple_fields_read_standard_lock_before_alias() {
    use maestro_app::browser::docx_preview::OpenXmlElementData as Data;
    for (attributes, expected) in [
        ("w:fldLock=\"true\" w:lock=\"false\"", true),
        ("w:fldLock=\"false\" w:lock=\"true\"", false),
        ("w:lock=\"true\"", true),
        ("w:fldLock=\"unknown\" w:lock=\"true\"", false),
    ] {
        let source = format!("<w:p xmlns:w=\"urn:w\"><w:fldSimple {attributes}/></w:p>");
        let xml = parse_xml_string(&source, true).unwrap();
        let tree = parse_paragraph(&OwnedXmlElement::new(xml.root())).unwrap();
        assert!(
            matches!(tree.nodes[1].data, Data::SimpleField { lock, dirty: false, .. } if lock == expected)
        );
    }
}

#[test]
fn complex_fields_read_standard_lock_before_alias() {
    use maestro_app::browser::docx_preview::OpenXmlElementData as Data;
    for (attributes, expected) in [
        ("w:fldLock=\"true\" w:lock=\"false\"", true),
        ("w:fldLock=\"false\" w:lock=\"true\"", false),
        ("w:lock=\"true\"", true),
        ("w:fldLock=\"unknown\" w:lock=\"true\"", false),
    ] {
        let source = format!("<w:r xmlns:w=\"urn:w\"><w:fldChar {attributes}/></w:r>");
        let xml = parse_xml_string(&source, true).unwrap();
        let tree = parse_run(&OwnedXmlElement::new(xml.root())).unwrap();
        assert!(
            matches!(tree.nodes[1].data, Data::ComplexField { lock, dirty: false, .. } if lock == expected)
        );
    }
}

#[test]
fn invalid_symbol_fonts_escape_style_element_delimiters() {
    use maestro_app::browser::docx_preview::OpenXmlElementData as Data;
    let xml = parse_xml_string("<r><sym font=\"A&lt;B\"/></r>", true).unwrap();
    let tree = parse_run(&OwnedXmlElement::new(xml.root())).unwrap();
    assert!(
        matches!(&tree.nodes[1].data, Data::Symbol { font: Some(font), .. } if font == "\"A\\3c B\"")
    );
}
