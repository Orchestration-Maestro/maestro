//! Package loading through the public Word reader.
#![cfg(test)]
use crate::docx_support;
use docx_support::cases;
use maestro_app::browser::docx_preview::{DocxError, Options, Relationship, WordDocument};
use serde::Deserialize;
use serde_json::{Value, json};

/// Supplied package and optional follow-up operations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Fixture relative to the format directory.
    file: String,
    /// Relationship IDs to resolve from the document.
    lookup: Option<Vec<String>>,
    /// Resource IDs to read from the document.
    resources: Option<Vec<String>>,
    /// Root relationship ID to resolve.
    #[serde(rename = "rootLookup")]
    root_lookup: Option<String>,
    /// Whether to inspect retained document XML.
    raw: Option<bool>,
}

/// Preserve absent, empty, repeated and ordered relationship records.
fn relationships(rels: Option<&[Relationship]>) -> Value {
    rels.map_or(Value::Null, |rels| {
        json!(
            rels.iter()
                .map(|r| json!({
                    "id": r.id, "type": r.r#type, "target": r.target, "targetMode": r.target_mode
                }))
                .collect::<Vec<_>>()
        )
    })
}

/// Observe exactly the operations requested by a package case.
fn observe(document: &WordDocument, input: &Input) -> Value {
    let mut observed = json!({
        "rels": relationships(document.rels()),
        "parts": document.parts().iter().map(|p| json!({
            "path": p.path, "kind": format!("{:?}Part", p.kind), "rels": relationships(p.rels.as_deref())
        })).collect::<Vec<_>>(),
        "document_path": document.document_part().map(|p| p.path.as_str())
    });
    if let Some(ids) = &input.lookup {
        let base = document.document_part().unwrap();
        let found: Vec<_> = ids
            .iter()
            .map(|id| document.find_part_by_rel_id(id, Some(base)))
            .collect();
        observed["lookup"] = json!(
            found
                .iter()
                .map(|part| part.map(|p| p.path.as_str()))
                .collect::<Vec<_>>()
        );
        assert!(std::ptr::eq(found[0].unwrap(), found[1].unwrap()));
    }
    if let Some(id) = &input.root_lookup {
        observed["root_lookup"] = json!(
            document
                .find_part_by_rel_id(id, None)
                .map(|p| p.path.as_str())
        );
    }
    if let Some(ids) = &input.resources {
        let base = document.document_part().unwrap();
        observed["resources"] = json!(
            ids.iter()
                .map(|id| json!({
                    "id": id, "path": document.get_path_by_id(base, id),
                    "bytes": document.load_resource(base, id).unwrap()
                }))
                .collect::<Vec<_>>()
        );
    }
    if input.raw.unwrap_or(false) {
        observed["xml"] = json!(document.document_part().unwrap().xml);
    }
    observed
}

/// Replay one package behavior against its controlled archives.
fn check(name: &str) {
    for case in cases::<Input>(include_str!("fixtures/docx/packages.json"), name) {
        let data = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/docx")
                .join(&case.input.file),
        )
        .unwrap();
        let result = WordDocument::load(&data, Options::default());
        let observed = match result {
            Ok(document) => observe(&document, &case.input),
            Err(DocxError::Archive(_)) => json!({"error": "Archive"}),
            Err(DocxError::Xml { path, .. }) => json!({"error": "Xml", "path": path}),
        };
        assert_eq!(observed, case.expected, "{}", case.input.file);
    }
}

#[test]
fn package_opens_stored_and_deflated_entries() {
    check("package_opens_stored_and_deflated_entries");
}

#[test]
fn document_uses_first_root_target_or_default() {
    check("document_uses_first_root_target_or_default");
}

#[test]
fn document_missing_parts_and_unknown_types_are_tolerated() {
    check("document_missing_parts_and_unknown_types_are_tolerated");
}

#[test]
fn document_missing_root_relationships_use_default_parts() {
    check("document_missing_root_relationships_use_default_parts");
}

#[test]
fn document_loads_all_supported_parts_before_returning() {
    check("document_loads_all_supported_parts_before_returning");
}

#[test]
fn document_related_xml_errors_are_eager() {
    check("document_related_xml_errors_are_eager");
}

#[test]
fn relationships_keep_order_duplicates_and_missing_attributes() {
    check("relationships_keep_order_duplicates_and_missing_attributes");
}

#[test]
fn document_cycles_and_repeated_targets_share_one_part() {
    check("document_cycles_and_repeated_targets_share_one_part");
}

#[test]
fn document_alias_keys_remain_distinct() {
    check("document_alias_keys_remain_distinct");
}

#[test]
fn package_lookup_prefers_slashes_then_backslashes() {
    check("package_lookup_prefers_slashes_then_backslashes");
}

#[test]
fn document_resources_are_read_only_when_requested() {
    check("document_resources_are_read_only_when_requested");
}

#[test]
fn document_root_lookup_works_without_a_base_part() {
    check("document_root_lookup_works_without_a_base_part");
}

#[test]
fn document_unknown_and_external_targets_are_not_fetched() {
    check("document_unknown_and_external_targets_are_not_fetched");
}

#[test]
fn package_invalid_archive_reports_archive_failure() {
    check("package_invalid_archive_reports_archive_failure");
}

#[test]
fn document_recursion_keeps_sibling_order_and_non_topological_edges() {
    check("document_recursion_keeps_sibling_order_and_non_topological_edges");
}

#[test]
fn document_relationship_failure_precedes_part_xml_failure() {
    check("document_relationship_failure_precedes_part_xml_failure");
}

#[test]
fn package_archive_names_are_sanitized_before_lookup() {
    check("package_archive_names_are_sanitized_before_lookup");
}

#[test]
fn document_xml_and_relationships_are_retained() {
    check("document_xml_and_relationships_are_retained");
}

#[test]
fn document_missing_part_relationships_fall_back_but_empty_ones_do_not() {
    let absent = WordDocument::load(
        include_bytes!("fixtures/docx/packages/root-fallback-absent.docx"),
        Options::default(),
    )
    .unwrap();
    let part = absent.document_part().unwrap();
    assert!(std::ptr::eq(
        absent.find_part_by_rel_id("root", Some(part)).unwrap(),
        part
    ));
    let empty = WordDocument::load(
        include_bytes!("fixtures/docx/packages/root-fallback-empty.docx"),
        Options::default(),
    )
    .unwrap();
    assert!(
        empty
            .find_part_by_rel_id("root", empty.document_part())
            .is_none()
    );
}

#[test]
fn package_collisions_select_the_last_central_directory_entry() {
    let document = WordDocument::load(
        include_bytes!("fixtures/docx/packages/central-order.docx"),
        Options::default(),
    )
    .unwrap();
    assert_eq!(
        document.document_part().unwrap().xml,
        "<document>C</document>"
    );
}

#[test]
fn document_external_paths_keep_the_resolved_url_identity() {
    let document = WordDocument::load(
        include_bytes!("fixtures/docx/packages/external-paths.docx"),
        Options::default(),
    )
    .unwrap();
    let part = document.document_part().unwrap();
    assert_eq!(
        document.get_path_by_id(part, "https").as_deref(),
        Some("https://example.org/a")
    );
    assert_eq!(
        document.get_path_by_id(part, "network").as_deref(),
        Some("http://elsewhere/a")
    );
}
