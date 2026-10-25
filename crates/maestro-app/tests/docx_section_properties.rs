//! Owned section records through the existing paragraph and body operations.
#![cfg(test)]
use crate::body_tree;
use crate::docx_support;
use docx_support::{XmlInput, cases};
use maestro_app::browser::docx_preview::{
    DocxError, OwnedXmlElement, parse_body_elements, parse_document_file, parse_xml_string,
};

/// Compare complete owned results after releasing input owners.
fn check(name: &str) {
    for (index, case) in
        cases::<XmlInput>(include_str!("fixtures/docx/section-properties.json"), name)
            .into_iter()
            .enumerate()
    {
        let tree = docx_support::framed(&case.input).unwrap();
        assert_eq!(body_tree::full_graph(&tree), case.expected, "case {index}");
    }
}
#[test]
fn section_geometry_uses_each_shared_length() {
    check("section_geometry_uses_each_shared_length");
}

#[test]
fn section_groups_keep_absence_empty_and_last_records() {
    check("section_groups_keep_absence_empty_and_last_records");
}

#[test]
fn section_types_and_orientation_keep_open_text() {
    check("section_types_and_orientation_keep_open_text");
}

#[test]
fn section_columns_keep_number_and_nested_order() {
    check("section_columns_keep_number_and_nested_order");
}

#[test]
fn section_booleans_keep_independent_defaults() {
    check("section_booleans_keep_independent_defaults");
}

#[test]
fn section_header_footer_references_keep_kind_and_order() {
    check("section_header_footer_references_keep_kind_and_order");
}

#[test]
fn section_page_numbers_keep_strings_and_numeric_prefixes() {
    check("section_page_numbers_keep_strings_and_numeric_prefixes");
}

#[test]
fn section_borders_keep_raw_fields_and_units() {
    check("section_borders_keep_raw_fields_and_units");
}

#[test]
fn section_borders_select_physical_sides_and_last_values() {
    check("section_borders_select_physical_sides_and_last_values");
}

#[test]
fn section_selection_uses_immediate_local_names() {
    check("section_selection_uses_immediate_local_names");
}

#[test]
fn section_presence_and_group_replacement_keep_other_metadata() {
    check("section_presence_and_group_replacement_keep_other_metadata");
}

#[test]
fn section_border_widths_clamp_without_changing_offsets() {
    check("section_border_widths_clamp_without_changing_offsets");
}

#[test]
fn section_border_flags_are_independent_and_nullable() {
    check("section_border_flags_are_independent_and_nullable");
}

#[test]
fn section_columns_convert_each_length_without_cross_wiring() {
    check("section_columns_convert_each_length_without_cross_wiring");
}

#[test]
fn section_parent_selection_uses_the_shared_qualified_owner() {
    check("section_parent_selection_uses_the_shared_qualified_owner");
}

#[test]
fn section_body_routes_keep_sections_and_hidden_parent_identity() {
    check("section_body_routes_keep_sections_and_hidden_parent_identity");
}

/// The document root keeps body content, the body's own section group and the background,
/// while a paragraph's section group stays with its paragraph.
#[test]
fn body_root_keeps_section_properties_and_background() {
    let source = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:background w:color="EEFFEE"/><w:body><w:p><w:pPr><w:sectPr><w:pgSz w:w="8391" w:h="11906"/></w:sectPr></w:pPr><w:r><w:t>one</w:t></w:r></w:p><w:p><w:r><w:t>two</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:header="720"/><w:headerReference w:type="default" r:id="rId1"/></w:sectPr></w:body></w:document>"#;
    let xml = parse_xml_string(source, false).unwrap();
    let body = parse_document_file(&OwnedXmlElement::new(xml.root()), false).unwrap();
    let content = xml.element(xml.root(), "body").unwrap();
    let direct = parse_body_elements(&OwnedXmlElement::new(content), false).unwrap();
    assert_eq!(
        body_tree::full_graph(&body.children),
        body_tree::full_graph(&direct)
    );
    assert_eq!(body.children.roots.len(), 2);
    let size = body.props.page_size.as_ref().unwrap();
    assert_eq!(
        (size.width.as_deref(), size.height.as_deref()),
        (Some("595.30pt"), Some("841.90pt"))
    );
    let margins = body.props.page_margins.as_ref().unwrap();
    assert_eq!(
        (margins.top.as_deref(), margins.header.as_deref()),
        (Some("72.00pt"), Some("36.00pt"))
    );
    let refs = body.props.header_refs.as_ref().unwrap();
    assert_eq!(
        refs.iter()
            .map(|r| (r.id.as_deref(), r.r#type.as_deref()))
            .collect::<Vec<_>>(),
        [(Some("rId1"), Some("default"))]
    );
    assert_eq!(
        body.css_style.into_iter().collect::<Vec<_>>(),
        [("background-color".to_owned(), Some("#EEFFEE".to_owned()))]
    );
    let bare = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body/></w:document>"#;
    let xml = parse_xml_string(bare, false).unwrap();
    let body = parse_document_file(&OwnedXmlElement::new(xml.root()), false).unwrap();
    assert!(body.children.roots.is_empty());
    assert!(body.props.page_size.is_none() && body.props.header_refs.is_none());
    assert!(body.css_style.is_empty());
    let bodiless =
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"/>"#;
    let xml = parse_xml_string(bodiless, false).unwrap();
    assert!(matches!(
        parse_document_file(&OwnedXmlElement::new(xml.root()), false),
        Err(DocxError::Xml { .. })
    ));
}

/// `SectionProperties`, `PageSize`, `PageMargins`, `Columns`, `Column`,
/// `FooterHeaderReference`, `PageNumber`, `Borders` and `Border` survive input release.
#[test]
fn section_records_outlive_all_xml_input_owners() {
    check("section_records_outlive_all_xml_input_owners");
}
