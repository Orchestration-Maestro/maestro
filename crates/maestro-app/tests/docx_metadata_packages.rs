//! Eager semantic preparation through the package reader.
#![cfg(test)]
use crate::body_tree;
use crate::docx_support;
use crate::metadata;
use docx_support::cases;
use maestro_app::browser::docx_preview::{Options, WordDocument};
use serde::Deserialize;
/// Composed package operation input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Operation independent of the behavior label.
    operation: String,
    /// Repository-relative controlled archive.
    file: String,
}
#[test]
fn document_prepares_metadata_and_comments_without_rendering() {
    for case in cases::<Input>(
        include_str!("fixtures/docx/metadata-packages.json"),
        "document_prepares_metadata_and_comments_without_rendering",
    ) {
        assert_eq!(case.input.operation, "package");
        let bytes = std::fs::read(
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/docx/").to_owned()
                + &case.input.file,
        )
        .unwrap();
        let doc = WordDocument::load(&bytes, Options::default()).unwrap();
        assert_eq!(metadata::package(&doc), case.expected);
    }
}
/// Replay the composed public operation, including alias and release actions.
fn check(name: &str) {
    use maestro_app::browser::docx_preview::DocxError;
    for case in cases::<Input>(include_str!("fixtures/docx/metadata-packages.json"), name) {
        let bytes = std::fs::read(
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/docx/").to_owned()
                + &case.input.file,
        )
        .unwrap();
        let doc = WordDocument::load(&bytes, Options::default());
        let observed = match doc {
            Err(DocxError::Xml { path, .. }) => {
                serde_json::json!({"kind":"invalid-xml-part","path":path})
            }
            Err(DocxError::Output(_)) => panic!("unexpected extracted-output failure"),
            Err(DocxError::FontKey) => panic!("Unexpected font-key error while parsing metadata"),
            Err(DocxError::Archive(_)) => panic!("unexpected archive error"),
            Ok(doc) => match case.input.operation.as_str() {
                "package" => metadata::package(&doc),
                "package_alias" => {
                    let mut observed = metadata::package(&doc);
                    let first = doc.find_part_by_rel_id("c", doc.document_part()).unwrap();
                    let second = doc
                        .find_part_by_rel_id("again", doc.document_part())
                        .unwrap();
                    assert!(std::ptr::eq(
                        first.data.as_ref().unwrap(),
                        second.data.as_ref().unwrap()
                    ));
                    observed["same_part"] = serde_json::json!(std::ptr::eq(first, second));
                    observed
                }
                "package_release" => {
                    let handle =
                        std::sync::Arc::clone(&doc.comments_part().unwrap().comment_map["7"]);
                    drop(doc);
                    drop(bytes);
                    let comment = handle.read().unwrap();
                    serde_json::json!({"author":comment.author,"children":body_tree::full_graph(&comment.children)})
                }
                _ => panic!("unknown package operation"),
            },
        };
        assert_eq!(observed, case.expected);
    }
}
#[test]
fn document_prepares_custom_values_after_nonvalue_nodes() {
    check("document_prepares_custom_values_after_nonvalue_nodes");
}
#[test]
fn document_prepared_parts_follow_cached_relationship_identity() {
    check("document_prepared_parts_follow_cached_relationship_identity");
}
#[test]
fn document_named_metadata_slots_select_last_loaded_kind() {
    check("document_named_metadata_slots_select_last_loaded_kind");
}
#[test]
fn document_semantic_failures_are_eager() {
    check("document_semantic_failures_are_eager");
}
#[test]
fn document_semantic_failure_precedes_related_part_failure() {
    check("document_semantic_failure_precedes_related_part_failure");
}
#[test]
fn document_admission_failure_precedes_semantic_preparation() {
    check("document_admission_failure_precedes_semantic_preparation");
}
#[test]
fn document_missing_semantic_parts_stay_absent() {
    check("document_missing_semantic_parts_stay_absent");
}
#[test]
fn package_comment_handles_survive_document_release() {
    check("package_comment_handles_survive_document_release");
}
