//! Rendered Word link targets and their refusal.
use super::check;
use crate::docx_preview::html_renderer::inline::hyperlink_href;
use maestro_app::browser::docx_preview::{Relationship, WmlHyperlink};
use serde::Deserialize;

/// One recorded link target decision.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Owning test.
    test: String,
    /// Link record and the document part's relationships.
    input: Input,
    /// Expected target, absent when refused.
    expected: Option<String>,
}

/// A link record and the document part's relationships.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Nullable relationship identifier.
    id: Option<String>,
    /// Nullable bookmark anchor.
    anchor: Option<String>,
    /// Ordered document-part relationships.
    relationships: Option<Vec<RecordedRelationship>>,
}

/// Relationship fields the link target reads.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordedRelationship {
    /// Case-sensitive identity.
    id: Option<String>,
    /// Authored target.
    target: Option<String>,
    /// Optional target mode.
    target_mode: Option<String>,
}

/// Replay every recorded decision owned by one test.
fn check_targets(name: &str) {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/docx/links.json")).unwrap();
    let selected: Vec<_> = cases.into_iter().filter(|case| case.test == name).collect();
    assert!(!selected.is_empty());
    for (index, case) in selected.into_iter().enumerate() {
        let link = WmlHyperlink {
            id: case.input.id,
            anchor: case.input.anchor,
        };
        let relationships: Option<Vec<Relationship>> = case.input.relationships.map(|list| {
            list.into_iter()
                .map(|rel| Relationship {
                    id: rel.id,
                    r#type: None,
                    target: rel.target,
                    target_mode: rel.target_mode,
                })
                .collect()
        });
        assert_eq!(
            hyperlink_href(&link, relationships.as_deref()),
            case.expected,
            "{name} case {index}"
        );
    }
}

#[test]
fn word_hyperlink_targets_follow_external_relationships_and_anchors() {
    check_targets("word_hyperlink_targets_follow_external_relationships_and_anchors");
}

#[test]
fn word_hyperlink_targets_refuse_executable_schemes() {
    check_targets("word_hyperlink_targets_refuse_executable_schemes");
}

#[test]
fn word_hyperlink_refusal_spares_inert_data_and_fragment_targets() {
    check_targets("word_hyperlink_refusal_spares_inert_data_and_fragment_targets");
}

/// Parsed links in a package reach the rendered pages, where a script target keeps its
/// link and text but carries no `href`.
#[test]
fn parsed_hyperlink_records_reach_the_link_policy() {
    check("hyperlink-targets");
}
