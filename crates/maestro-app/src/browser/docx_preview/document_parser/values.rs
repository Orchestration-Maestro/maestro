//! Border shorthand values.
use super::super::{LengthUsage, XmlElement, XmlParser};
use super::xml_util::color_attr;

/// Render one border, leaving absent components to CSS defaults.
pub(super) fn value_of_border<'doc, 'input: 'doc>(
    xml: &'doc XmlParser<'input>,
    element: XmlElement<'doc, 'input>,
) -> String {
    let kind = parse_border_type(xml.attr(element, "val"));
    if kind == "none" {
        return kind.into();
    }
    let size = xml.length_attr(element, "sz", LengthUsage::Border);
    let color = color_attr(xml, element, "color");
    [size.as_deref(), Some(kind), color.as_deref()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Select the CSS style for a document border token.
fn parse_border_type(value: Option<&str>) -> &'static str {
    match value {
        Some("dashed" | "dashSmallGap") => "dashed",
        Some("dotDash" | "dotDotDash" | "dotted") => "dotted",
        Some("double" | "doubleWave" | "triple") => "double",
        Some("inset") => "inset",
        Some("nil" | "none") => "none",
        Some("outset") => "outset",
        _ => "solid",
    }
}
