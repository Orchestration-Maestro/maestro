//! Word tables, rows, cells and inline wrappers in the owned element tree.
#![cfg(test)]
use crate::body_tree;
use crate::docx_support::{self, XmlInput, cases};
use maestro_app::browser::docx_preview::{
    CommentsPart, EndnotesPart, FooterPart, FootnotesPart, HeaderPart, OpenXmlElementData as Data,
    OpenXmlElementTree, Options, OwnedXmlElement, WordDocument, parse_body_elements,
    parse_paragraph, parse_run, parse_xml_string,
};

/// Replay every recorded case owned by one behavior.
fn check(name: &str) {
    for case in cases::<XmlInput>(include_str!("fixtures/docx/tables.json"), name) {
        let tree = docx_support::framed(&case.input).unwrap();
        assert_eq!(
            body_tree::full_graph(&tree),
            case.expected,
            "{}",
            case.input.xml
        );
    }
}

#[test]
fn table_children_keep_cell_and_nested_table_order() {
    check("table_children_keep_cell_and_nested_table_order");
}

#[test]
fn table_grids_keep_absence_empty_and_last_group() {
    check("table_grids_keep_absence_empty_and_last_group");
}

#[test]
fn column_widths_use_shared_lengths() {
    check("column_widths_use_shared_lengths");
}

#[test]
fn table_grids_use_immediate_local_names() {
    check("table_grids_use_immediate_local_names");
}

#[test]
fn table_property_groups_reset_styles_not_metadata() {
    check("table_property_groups_reset_styles_not_metadata");
}

#[test]
fn table_properties_keep_style_and_band_records() {
    check("table_properties_keep_style_and_band_records");
}

#[test]
fn table_alignment_adjusts_only_center_and_right() {
    check("table_alignment_adjusts_only_center_and_right");
}

#[test]
fn table_visibility_uses_hidden_presence() {
    check("table_visibility_uses_hidden_presence");
}

#[test]
fn table_positions_keep_each_independent_margin() {
    check("table_positions_keep_each_independent_margin");
}

#[test]
fn table_positions_accumulate_before_alignment() {
    check("table_positions_accumulate_before_alignment");
}

#[test]
fn table_look_keeps_order_and_independent_flags() {
    check("table_look_keeps_order_and_independent_flags");
}

#[test]
fn table_look_masks_remain_independent_of_flags() {
    check("table_look_masks_remain_independent_of_flags");
}

#[test]
fn table_look_uses_shared_hex_prefixes() {
    check("table_look_uses_shared_hex_prefixes");
}

#[test]
fn row_property_aliases_keep_order_and_reset_styles() {
    check("row_property_aliases_keep_order_and_reset_styles");
}

#[test]
fn row_headers_keep_nullable_boolean_values() {
    check("row_headers_keep_nullable_boolean_values");
}

#[test]
fn row_grid_offsets_keep_numeric_results() {
    check("row_grid_offsets_keep_numeric_results");
}

#[test]
fn conditional_classes_delegate_to_the_shared_owner() {
    check("conditional_classes_delegate_to_the_shared_owner");
}

#[test]
fn conditional_attribute_flags_select_classes() {
    check("conditional_attribute_flags_select_classes");
}

#[test]
fn cell_spans_keep_missing_empty_zero_and_prefixes() {
    check("cell_spans_keep_missing_empty_zero_and_prefixes");
}

#[test]
fn cell_merges_keep_absent_empty_and_open_values() {
    check("cell_merges_keep_absent_empty_and_open_values");
}

#[test]
fn cell_directions_keep_fallback_and_last_override() {
    check("cell_directions_keep_fallback_and_last_override");
}

#[test]
fn cell_property_groups_reset_styles_not_span_or_merge() {
    check("cell_property_groups_reset_styles_not_span_or_merge");
}

#[test]
fn table_border_and_margin_routes_remain_distinct() {
    check("table_border_and_margin_routes_remain_distinct");
}

#[test]
fn table_and_cell_width_options_reach_the_dispatcher() {
    check("table_and_cell_width_options_reach_the_dispatcher");
}

#[test]
fn hyperlinks_keep_id_anchor_and_parent_identity() {
    check("hyperlinks_keep_id_anchor_and_parent_identity");
}

#[test]
fn smart_tags_keep_nonempty_metadata_and_parent_identity() {
    check("smart_tags_keep_nonempty_metadata_and_parent_identity");
}

#[test]
fn inline_wrappers_accept_only_immediate_runs() {
    check("inline_wrappers_accept_only_immediate_runs");
}

#[test]
fn inline_wrapper_parents_survive_controls_and_changes() {
    check("inline_wrapper_parents_survive_controls_and_changes");
}

#[test]
fn inline_wrappers_share_local_name_attribute_selection() {
    check("inline_wrappers_share_local_name_attribute_selection");
}

#[test]
fn inline_wrappers_keep_paragraph_marker_order() {
    check("inline_wrappers_keep_paragraph_marker_order");
}

#[test]
fn body_content_controls_reach_the_table_parser() {
    check("body_content_controls_reach_the_table_parser");
}

#[test]
fn producer_tables_match_the_recorded_body() {
    let document = WordDocument::load(
        include_bytes!("fixtures/docx/packages/table-contracts.docx"),
        Options::default(),
    )
    .unwrap();
    let source = &document.document_part().unwrap().xml;
    for case in cases::<XmlInput>(
        include_str!("fixtures/docx/tables.json"),
        "producer_tables_match_the_recorded_body",
    ) {
        assert_eq!(&case.input.xml, source);
    }
    check("producer_tables_match_the_recorded_body");
}

/// One table whose cell width only the default policy keeps.
const TABLE: &str = r#"<w:tbl><w:tblPr><w:tblW w:w="400" w:type="dxa"/></w:tblPr><w:tr><w:tc><w:tcPr><w:tcW w:w="200" w:type="dxa"/></w:tcPr></w:tc></w:tr></w:tbl>"#;

/// Wrap content in a namespaced root element.
fn rooted(tag: &str, content: &str) -> String {
    format!(r#"<w:{tag} xmlns:w="urn:w" xmlns:v="urn:v">{content}</w:{tag}>"#)
}

/// Parse one route's root with the selected width policy.
fn route_tree(route: &str, ignore_width: bool) -> OpenXmlElementTree {
    let textbox = format!(
        "<w:pict><v:shape><v:textbox><w:txbxContent>{TABLE}</w:txbxContent></v:textbox></v:shape></w:pict>"
    );
    let source = match route {
        "body" => rooted("body", TABLE),
        "paragraph" => rooted("p", &format!("<w:r>{textbox}</w:r>")),
        "run" => rooted("r", &textbox),
        "header" => rooted("hdr", TABLE),
        "footer" => rooted("ftr", TABLE),
        "footnotes" => rooted("footnotes", &format!("<w:footnote>{TABLE}</w:footnote>")),
        "endnotes" => rooted("endnotes", &format!("<w:endnote>{TABLE}</w:endnote>")),
        _ => rooted("comments", &format!("<w:comment>{TABLE}</w:comment>")),
    };
    let xml = parse_xml_string(&source, false).unwrap();
    let input = OwnedXmlElement::new(xml.root());
    match route {
        "body" => parse_body_elements(&input, ignore_width).unwrap(),
        "paragraph" => parse_paragraph(&input, ignore_width).unwrap(),
        "run" => parse_run(&input, ignore_width).unwrap(),
        "header" => {
            HeaderPart::parse_xml(&input, ignore_width)
                .unwrap()
                .root_element
                .children
        }
        "footer" => {
            FooterPart::parse_xml(&input, ignore_width)
                .unwrap()
                .root_element
                .children
        }
        "footnotes" => {
            let mut part = FootnotesPart::parse_xml(&input, ignore_width).unwrap();
            part.notes.remove(0).base.children
        }
        "endnotes" => {
            let mut part = EndnotesPart::parse_xml(&input, ignore_width).unwrap();
            part.notes.remove(0).base.children
        }
        _ => {
            let part = CommentsPart::parse_xml(&input, ignore_width).unwrap();
            let mut comment = part.comments[0].write().unwrap();
            std::mem::take(&mut comment.children)
        }
    }
}

/// Read the table and cell widths of a one-cell table.
fn widths(tree: &OpenXmlElementTree) -> (Option<String>, Option<String>) {
    let width = |style: Option<&indexmap::IndexMap<String, Option<String>>>| {
        style.unwrap().get("width").cloned().flatten()
    };
    let mut widths = (None, None);
    for node in &tree.nodes {
        match &node.data {
            Data::Table(t) => widths.0 = width(t.css_style.as_ref()),
            Data::Cell(c) => widths.1 = width(c.css_style.as_ref()),
            _ => {}
        }
    }
    widths
}

#[test]
fn width_policy_reaches_every_body_route() {
    for route in [
        "body",
        "paragraph",
        "run",
        "header",
        "footer",
        "footnotes",
        "endnotes",
        "comments",
    ] {
        for (ignore_width, cell) in [(false, Some("10.00pt")), (true, None)] {
            assert_eq!(
                widths(&route_tree(route, ignore_width)),
                (Some("20.00pt".to_owned()), cell.map(str::to_owned)),
                "{route} {ignore_width}"
            );
        }
    }
}
