//! Comment factories, shared records and typed markers.
#![cfg(test)]
#[path = "docx_support/body_tree.rs"]
pub mod body_tree;
mod docx_support;
#[path = "docx_support/metadata.rs"]
pub mod metadata;
use docx_support::cases;
use maestro_app::browser::docx_preview::{CommentsPart, OwnedXmlElement, parse_xml_string};
use serde::Deserialize;
/// Public factory or body operation input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Operation, independent of behavior labels.
    operation: String,
    /// Complete admitted source.
    xml: String,
}
#[test]
fn comments_preserve_metadata_order_and_duplicate_ids() {
    for case in cases::<Input>(
        include_str!("fixtures/docx/comments.json"),
        "comments_preserve_metadata_order_and_duplicate_ids",
    ) {
        assert_eq!(case.input.operation, "comments");
        let xml = parse_xml_string(&case.input.xml, true).unwrap();
        let part = CommentsPart::parse_xml(&OwnedXmlElement::new(xml.root())).unwrap();
        assert_eq!(metadata::comments(&part), case.expected);
    }
}

/// Compare plain comment factories against the complete controlled observation.
fn check_comments(name: &str) {
    for case in cases::<Input>(include_str!("fixtures/docx/comments.json"), name) {
        assert_eq!(case.input.operation, "comments");
        let xml = parse_xml_string(&case.input.xml, true).unwrap();
        let part = CommentsPart::parse_xml(&OwnedXmlElement::new(xml.root())).unwrap();
        assert_eq!(metadata::comments(&part), case.expected);
    }
}
#[test]
fn comments_choose_immediate_local_names_and_first_attributes() {
    check_comments("comments_choose_immediate_local_names_and_first_attributes");
}
#[test]
fn comment_children_reuse_formatting_and_parent_identity() {
    check_comments("comment_children_reuse_formatting_and_parent_identity");
}
#[test]
fn comment_parts_keep_source_after_input_release() {
    for case in cases::<Input>(
        include_str!("fixtures/docx/comments.json"),
        "comment_parts_keep_source_after_input_release",
    ) {
        assert_eq!(case.input.operation, "comments");
        let part = {
            let xml = parse_xml_string(&case.input.xml, true).unwrap();
            let input = OwnedXmlElement::new(xml.root());
            CommentsPart::parse_xml(&input).unwrap()
        };
        drop(case.input);
        assert_eq!(metadata::comments(&part), case.expected);
    }
}
#[test]
fn comment_lookup_shares_records_with_source_list() {
    for case in cases::<Input>(
        include_str!("fixtures/docx/comments.json"),
        "comment_lookup_shares_records_with_source_list",
    ) {
        assert_eq!(case.input.operation, "comment_alias");
        let xml = parse_xml_string(&case.input.xml, true).unwrap();
        let mut part = CommentsPart::parse_xml(&OwnedXmlElement::new(xml.root())).unwrap();
        let handle = std::sync::Arc::clone(&part.comment_map["2"]);
        let same = std::sync::Arc::ptr_eq(&handle, &part.comments[2]);
        {
            let mut c = handle.write().unwrap();
            c.author = Some("changed through lookup".into());
            c.id = Some("changed-id".into());
        }
        let list_author = part.comments[2].read().unwrap().author.clone();
        let old_key_still_alias = std::sync::Arc::ptr_eq(&handle, &part.comment_map["2"]);
        let new_key_absent = !part.comment_map.contains_key("changed-id");
        let lookup_keys: Vec<_> = part.comment_map.keys().cloned().collect();
        part.comments.clear();
        let c = handle.read().unwrap();
        assert_eq!(
            serde_json::json!({"same":same,"list_author":list_author,"retained_author":c.author,"retained_id":c.id,"old_key_still_alias":old_key_still_alias,"new_key_absent":new_key_absent,"lookup_keys":lookup_keys}),
            case.expected
        );
    }
}
#[test]
fn comment_extensions_preserve_links_and_nullable_flags() {
    for case in cases::<Input>(
        include_str!("fixtures/docx/comments.json"),
        "comment_extensions_preserve_links_and_nullable_flags",
    ) {
        assert_eq!(case.input.operation, "extensions");
        let xml = parse_xml_string(&case.input.xml, true).unwrap();
        let part =
            maestro_app::browser::docx_preview::CommentsExtendedPart::parse_xml(&xml, xml.root());
        assert_eq!(metadata::extensions(&part), case.expected);
    }
}
/// Compare extended records using the public factory.
fn check_extensions(name: &str) {
    for case in cases::<Input>(include_str!("fixtures/docx/comments.json"), name) {
        assert_eq!(case.input.operation, "extensions");
        let xml = parse_xml_string(&case.input.xml, true).unwrap();
        let part =
            maestro_app::browser::docx_preview::CommentsExtendedPart::parse_xml(&xml, xml.root());
        assert_eq!(metadata::extensions(&part), case.expected);
    }
}
#[test]
fn comment_extension_flags_use_exact_tokens() {
    check_extensions("comment_extension_flags_use_exact_tokens");
}
#[test]
fn comment_extension_lookup_replaces_duplicates_without_losing_records() {
    check_extensions("comment_extension_lookup_replaces_duplicates_without_losing_records");
}
#[test]
fn comment_extension_lookup_shares_retained_record() {
    for case in cases::<Input>(
        include_str!("fixtures/docx/comments.json"),
        "comment_extension_lookup_shares_retained_record",
    ) {
        assert_eq!(case.input.operation, "extension_alias");
        let xml = parse_xml_string(&case.input.xml, true).unwrap();
        let mut part =
            maestro_app::browser::docx_preview::CommentsExtendedPart::parse_xml(&xml, xml.root());
        let handle = std::sync::Arc::clone(&part.comment_map["A"]);
        let same = std::sync::Arc::ptr_eq(&handle, &part.comments[0]);
        {
            let mut c = handle.write().unwrap();
            c.done = Some(true);
            c.para_id = Some("changed-id".into());
        }
        let list_done = part.comments[0].read().unwrap().done;
        let old_key_still_alias = std::sync::Arc::ptr_eq(&handle, &part.comment_map["A"]);
        let new_key_absent = !part.comment_map.contains_key("changed-id");
        let lookup_keys: Vec<_> = part.comment_map.keys().cloned().collect();
        part.comments.clear();
        let c = handle.read().unwrap();
        assert_eq!(
            serde_json::json!({"same":same,"list_done":list_done,"retained_done":c.done,"retained_id":c.para_id,"old_key_still_alias":old_key_still_alias,"new_key_absent":new_key_absent,"lookup_keys":lookup_keys}),
            case.expected
        );
    }
}
#[test]
fn comment_body_errors_fail_the_enclosing_part() {
    for case in cases::<Input>(
        include_str!("fixtures/docx/comments.json"),
        "comment_body_errors_fail_the_enclosing_part",
    ) {
        assert_eq!(case.input.operation, "comments");
        let xml = parse_xml_string(&case.input.xml, true).unwrap();
        assert!(matches!(
            CommentsPart::parse_xml(&OwnedXmlElement::new(xml.root())),
            Err(maestro_app::browser::docx_preview::DocxError::Xml { .. })
        ));
        assert_eq!(
            case.expected,
            serde_json::json!({"kind":"invalid-xml-part"})
        );
    }
}
#[test]
fn comment_markers_keep_kind_id_and_inline_order() {
    use maestro_app::browser::docx_preview::{
        OpenXmlElementData as Data, parse_paragraph, parse_run,
    };
    for case in cases::<Input>(
        include_str!("fixtures/docx/comments.json"),
        "comment_markers_keep_kind_id_and_inline_order",
    ) {
        let xml = parse_xml_string(&case.input.xml, true).unwrap();
        let input = OwnedXmlElement::new(xml.root());
        let tree = match case.input.operation.as_str() {
            "run" => parse_run(&input).unwrap(),
            "paragraph" => parse_paragraph(&input).unwrap(),
            _ => panic!("unexpected marker operation"),
        };
        assert!(tree.nodes.iter().any(|n| matches!(
            n.data,
            Data::CommentReference(_) | Data::CommentRangeStart(_) | Data::CommentRangeEnd(_)
        )));
        assert_eq!(body_tree::full_graph(&tree), case.expected);
    }
}
#[test]
fn comment_markers_survive_nested_body_routes() {
    check_comments("comment_markers_survive_nested_body_routes");
}
