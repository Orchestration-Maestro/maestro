//! Document styles as rules under the class namespace.
use super::check_preview;
use crate::docx_preview::html_renderer::{Styles, steps_taken};
use maestro_app::browser::docx_preview::{StylesPart, parse_xml_string};

#[test]
fn document_styles_emit_inherited_linked_default_and_repeated_rules() {
    check_preview("document-styles");
}

#[test]
fn style_rules_with_unusable_class_names_are_skipped() {
    check_preview("style-selector-escape");
}

/// A long base chain resolves the root's values once per definition, a base cycle keeps
/// each start's values until the first repeat, and the traversal takes one step per
/// definition.
#[test]
fn long_and_cyclic_base_chains_resolve_from_unchanged_definitions() {
    const NUM: &str = r#"<w:numPr><w:numId w:val="7"/><w:ilvl w:val="2"/></w:numPr>"#;
    let chain: String = std::iter::once(format!(
        r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:style w:type="paragraph" w:styleId="S0"><w:pPr>{NUM}</w:pPr></w:style>"#
    ))
    .chain((1..2_000).map(|index| {
        format!(
            r#"<w:style w:type="paragraph" w:styleId="S{index}"><w:basedOn w:val="S{}"/></w:style>"#,
            index - 1
        )
    }))
    .chain(std::iter::once("</w:styles>".to_owned()))
    .collect();
    let parsed = styles(&chain);
    let steps = steps_taken();
    assert!(
        steps <= 2_000,
        "{steps} base-chain steps for 2000 definitions"
    );
    let numbering = |name| {
        let numbering = parsed
            .find_style(Some(name))
            .unwrap()
            .numbering
            .as_ref()
            .unwrap();
        (
            numbering.id.as_ref().and_then(Option::as_deref),
            numbering.level.flatten(),
        )
    };
    assert_eq!(numbering("S1999"), (Some("7"), Some(2.0)));
    assert_eq!(numbering("S999"), (Some("7"), Some(2.0)));
    let cycle = r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:style w:type="paragraph" w:styleId="A"><w:basedOn w:val="B"/><w:pPr><w:numPr><w:numId w:val="1"/></w:numPr></w:pPr></w:style>
<w:style w:type="paragraph" w:styleId="B"><w:basedOn w:val="C"/></w:style>
<w:style w:type="paragraph" w:styleId="C"><w:basedOn w:val="A"/><w:pPr><w:numPr><w:numId w:val="3"/></w:numPr></w:pPr></w:style>
</w:styles>"#;
    let parsed = styles(cycle);
    let numbering = |name| {
        parsed
            .find_style(Some(name))
            .unwrap()
            .numbering
            .as_ref()
            .unwrap()
            .id
            .as_ref()
            .and_then(Option::as_deref)
    };
    assert_eq!(numbering("A"), Some("1"));
    assert_eq!(numbering("B"), Some("3"));
    assert_eq!(numbering("C"), Some("3"));
}

/// Resolve one styles part from its XML.
fn styles(source: &str) -> Styles {
    let xml = parse_xml_string(source, false).unwrap();
    let parsed = StylesPart::parse_xml(&xml, xml.root(), false).unwrap();
    Styles::process_styles(&parsed.styles)
}
