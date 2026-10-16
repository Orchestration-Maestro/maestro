//! Owned paragraph metadata and immediate property readers.
use super::super::{XmlElement, XmlParser};
use super::common::{CommonProperties, LengthUsage, WORDML_NAMESPACE, parse_common_property};
/// One tab stop, including nullable attributes.
#[derive(Debug, Default)]
pub struct TabStop {
    /// Converted tab position.
    pub position: Option<String>,
    /// Open tab leader spelling.
    pub leader: Option<String>,
    /// Open tab style spelling.
    pub style: Option<String>,
}
/// Numbering reference metadata.
#[derive(Debug, Default)]
pub struct Numbering {
    /// Encountered raw numbering identifier.
    pub id: Option<Option<String>>,
    /// Encountered numbering level.
    pub level: Option<Option<f64>>,
}
/// Encountered line spacing metadata.
#[derive(Debug, Default)]
pub struct LineSpacing {
    /// Converted spacing before the paragraph.
    pub before: Option<String>,
    /// Converted spacing after the paragraph.
    pub after: Option<String>,
    /// Numeric line spacing.
    pub line: Option<f64>,
    /// Open line spacing rule.
    pub line_rule: Option<String>,
}
/// Owned paragraph properties; outer options record property encounters.
#[derive(Debug, Default)]
pub struct ParagraphProperties {
    /// Common raw paragraph metadata.
    pub common: CommonProperties,
    /// Fresh ordered CSS from the last property group.
    pub css_style: Option<indexmap::IndexMap<String, Option<String>>>,
    /// Encountered tab stops, including an empty group.
    pub tabs: Option<Vec<TabStop>>,
    /// Encountered numbering reference.
    pub numbering: Option<Numbering>,
    /// Encountered line spacing.
    pub line_spacing: Option<LineSpacing>,
    /// Encountered raw text alignment.
    pub text_alignment: Option<Option<String>>,
    /// Encountered keep-lines flag.
    pub keep_lines: Option<bool>,
    /// Encountered keep-next flag.
    pub keep_next: Option<bool>,
    /// Encountered page-break-before flag.
    pub page_break_before: Option<bool>,
    /// Encountered outline level.
    pub outline_level: Option<Option<f64>>,
    /// Encountered paragraph style reference.
    pub style_name: Option<Option<String>>,
    /// Common metadata from the last nested run property group.
    pub run_props: Option<CommonProperties>,
    /// Encountered conditional classes in declared order.
    pub class_name: Option<String>,
}

/// Read one qualified metadata child, preserving CSS fallthrough for shared writers.
pub(crate) fn parse_paragraph_property(
    xml: &XmlParser<'_>,
    element: XmlElement<'_, '_>,
    props: &mut ParagraphProperties,
) -> bool {
    if element.namespace() != Some(WORDML_NAMESPACE) {
        return false;
    }
    if parse_common_property(xml, element, &mut props.common) {
        return true;
    }
    match element.local_name() {
        "keepLines" => props.keep_lines = xml.bool_attr(element, "val", Some(true)),
        "keepNext" => props.keep_next = xml.bool_attr(element, "val", Some(true)),
        "pageBreakBefore" => props.page_break_before = xml.bool_attr(element, "val", Some(true)),
        "numPr" => props.numbering = Some(parse_numbering(xml, element)),
        "outlineLvl" => props.outline_level = Some(xml.int_attr(element, "val", None)),
        "tabs" => props.tabs = Some(parse_tabs(xml, element)),
        "spacing" => {
            props.line_spacing = Some(parse_line_spacing(xml, element));
            return false;
        }
        "textAlignment" => {
            props.text_alignment = Some(xml.attr(element, "val").map(str::to_owned));
            return false;
        }
        "pStyle" => props.style_name = Some(xml.attr(element, "val").map(str::to_owned)),
        "rPr" => props.run_props = Some(super::run::parse_run_properties(xml, element)),
        _ => return false,
    }
    true
}

/// Read immediate local-name numbering fields in encounter order.
fn parse_numbering(xml: &XmlParser<'_>, element: XmlElement<'_, '_>) -> Numbering {
    let mut result = Numbering::default();
    for child in xml.elements(element, None) {
        match child.local_name() {
            "numId" => result.id = Some(xml.attr(child, "val").map(str::to_owned)),
            "ilvl" => result.level = Some(xml.int_attr(child, "val", None)),
            _ => {}
        }
    }
    result
}

/// Read immediate tab records in source order.
fn parse_tabs(xml: &XmlParser<'_>, element: XmlElement<'_, '_>) -> Vec<TabStop> {
    xml.elements(element, Some("tab"))
        .map(|child| TabStop {
            position: xml.length_attr(child, "pos", LengthUsage::Dxa),
            leader: xml.attr(child, "leader").map(str::to_owned),
            style: xml.attr(child, "val").map(str::to_owned),
        })
        .collect()
}
/// Read raw paragraph spacing alongside the shared CSS writer.
fn parse_line_spacing(xml: &XmlParser<'_>, element: XmlElement<'_, '_>) -> LineSpacing {
    LineSpacing {
        before: xml.length_attr(element, "before", LengthUsage::Dxa),
        after: xml.length_attr(element, "after", LengthUsage::Dxa),
        line: xml.int_attr(element, "line", None),
        line_rule: xml.attr(element, "lineRule").map(str::to_owned),
    }
}

/// Collect fresh raw paragraph metadata from immediate children.
pub(crate) fn parse_paragraph_properties(
    xml: &XmlParser<'_>,
    element: XmlElement<'_, '_>,
) -> ParagraphProperties {
    let mut result = ParagraphProperties::default();
    for child in xml.elements(element, None) {
        parse_paragraph_property(xml, child, &mut result);
    }
    result
}
