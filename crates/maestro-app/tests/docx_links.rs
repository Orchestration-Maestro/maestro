//! Rendered Word hyperlink targets and their refusal.
#![cfg(test)]
use crate::docx_support;
use docx_support::cases;
use maestro_app::browser::docx_preview::{OpenXmlElementData, Relationship, WmlHyperlink};
use maestro_app::browser::hyperlink_href;
use serde::Deserialize;
use serde_json::json;

/// Hyperlink record and the document part's relationships from the controlled corpus.
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

/// Retained relationship fields the link policy reads.
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

impl RecordedRelationship {
    /// The landed record with the relationship type left open.
    fn into_record(self) -> Relationship {
        Relationship {
            id: self.id,
            r#type: None,
            target: self.target,
            target_mode: self.target_mode,
        }
    }
}

/// Replay the link policy over one behavior group.
fn check(name: &str) {
    for case in cases::<Input>(include_str!("fixtures/docx/links.json"), name) {
        let link = WmlHyperlink {
            id: case.input.id,
            anchor: case.input.anchor,
        };
        let relationships: Option<Vec<Relationship>> = case.input.relationships.map(|list| {
            list.into_iter()
                .map(RecordedRelationship::into_record)
                .collect()
        });
        assert_eq!(
            json!(hyperlink_href(&link, relationships.as_deref())),
            case.expected,
            "{name}"
        );
    }
}

#[test]
fn word_hyperlink_targets_follow_external_relationships_and_anchors() {
    check("word_hyperlink_targets_follow_external_relationships_and_anchors");
}

#[test]
fn word_hyperlink_targets_refuse_executable_schemes() {
    check("word_hyperlink_targets_refuse_executable_schemes");
}

#[test]
fn word_hyperlink_refusal_spares_inert_data_and_fragment_targets() {
    check("word_hyperlink_refusal_spares_inert_data_and_fragment_targets");
}

/// An external relationship whose identifier is `id` and whose target is `target`.
fn external(id: &str, target: &str) -> Relationship {
    Relationship {
        id: Some(id.to_owned()),
        r#type: None,
        target: Some(target.to_owned()),
        target_mode: Some("External".to_owned()),
    }
}

#[test]
fn parsed_hyperlink_records_reach_the_link_policy() {
    let xml = concat!(
        r#"<w:body xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#,
        r#" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">"#,
        r#"<w:p><w:hyperlink r:id="rId9" w:anchor="here"><w:r><w:t>external</w:t></w:r></w:hyperlink></w:p>"#,
        r#"<w:p><w:hyperlink r:id="rId10"><w:r><w:t>script</w:t></w:r></w:hyperlink></w:p>"#,
        r#"<w:p><w:hyperlink w:anchor="only"><w:r><w:t>anchor</w:t></w:r></w:hyperlink></w:p>"#,
        "</w:body>",
    );
    let tree = docx_support::framed(&docx_support::XmlInput {
        operation: "body".to_owned(),
        xml: xml.to_owned(),
        ignore_width: false,
    })
    .unwrap();
    let relationships = [
        external("rId9", "https://example.test/page"),
        external("rId10", "javascript:alert(1)"),
    ];
    let hrefs: Vec<_> = tree
        .nodes
        .iter()
        .filter_map(|node| match &node.data {
            OpenXmlElementData::Hyperlink(link) => Some(hyperlink_href(link, Some(&relationships))),
            _ => None,
        })
        .collect();
    assert_eq!(
        hrefs,
        vec![
            Some("https://example.test/page#here".to_owned()),
            None,
            Some("#only".to_owned())
        ]
    );
}
