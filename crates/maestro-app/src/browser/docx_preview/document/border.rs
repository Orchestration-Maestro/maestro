//! Owned raw page borders, separate from CSS border interpretation.
use super::super::{XmlElement, XmlParser};
use super::common::LengthUsage;

/// Raw border attributes with converted dimensions.
#[derive(Debug, Default)]
pub struct Border {
    /// Open border type spelling.
    pub r#type: Option<String>,
    /// Literal color attribute.
    pub color: Option<String>,
    /// Converted border width.
    pub size: Option<String>,
    /// Converted point offset.
    pub offset: Option<String>,
    /// Nullable frame flag.
    pub frame: Option<bool>,
    /// Nullable shadow flag.
    pub shadow: Option<bool>,
}
/// Encountered physical page-border sides.
#[derive(Debug, Default)]
pub struct Borders {
    /// Last left border.
    pub left: Option<Border>,
    /// Last top border.
    pub top: Option<Border>,
    /// Last right border.
    pub right: Option<Border>,
    /// Last bottom border.
    pub bottom: Option<Border>,
}

/// Read literal border fields through the shared scalar owners.
fn parse_border(xml: &XmlParser<'_>, element: XmlElement<'_, '_>) -> Border {
    Border {
        r#type: xml.attr(element, "val").map(str::to_owned),
        color: xml.attr(element, "color").map(str::to_owned),
        size: xml.length_attr(element, "sz", LengthUsage::Border),
        offset: xml.length_attr(element, "space", LengthUsage::Point),
        frame: xml.bool_attr(element, "frame", None),
        shadow: xml.bool_attr(element, "shadow", None),
    }
}
/// Read immediate physical sides, replacing repeated sides completely.
pub(crate) fn parse_borders(xml: &XmlParser<'_>, element: XmlElement<'_, '_>) -> Borders {
    let mut borders = Borders::default();
    for child in xml.elements(element, None) {
        match child.local_name() {
            "left" => borders.left = Some(parse_border(xml, child)),
            "top" => borders.top = Some(parse_border(xml, child)),
            "right" => borders.right = Some(parse_border(xml, child)),
            "bottom" => borders.bottom = Some(parse_border(xml, child)),
            _ => {}
        }
    }
    borders
}
