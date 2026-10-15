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
    let color = color_attr(xml, element, "color", "black");
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

/// Select horizontal alignment, retaining open values and absence.
pub(super) fn value_of_jc<'doc, 'input: 'doc>(
    xml: &'doc XmlParser<'input>,
    element: XmlElement<'doc, 'input>,
) -> Option<String> {
    xml.attr(element, "val").map(|value| {
        match value {
            "start" | "left" => "left",
            "end" | "right" => "right",
            "center" => "center",
            "both" => "justify",
            _ => value,
        }
        .into()
    })
}
/// Select vertical alignment, retaining open values and absence.
pub(super) fn value_of_text_alignment<'doc, 'input: 'doc>(
    xml: &'doc XmlParser<'input>,
    element: XmlElement<'doc, 'input>,
) -> Option<String> {
    xml.attr(element, "val").map(|value| {
        match value {
            "auto" | "baseline" => "baseline",
            "top" => "top",
            "center" => "middle",
            "bottom" => "bottom",
            _ => value,
        }
        .into()
    })
}

/// Select a width's unit policy or its automatic value.
pub(super) fn value_of_size<'doc, 'input: 'doc>(
    xml: &'doc XmlParser<'input>,
    element: XmlElement<'doc, 'input>,
    attr: &str,
) -> Option<String> {
    let usage = match xml.attr(element, "type") {
        Some("auto") => return Some("auto".into()),
        Some("pct") => LengthUsage::Percent,
        _ => LengthUsage::Dxa,
    };
    xml.length_attr(element, attr, usage)
}

/// Read a physical margin using twentieths of a point.
pub(super) fn value_of_margin<'doc, 'input: 'doc>(
    xml: &'doc XmlParser<'input>,
    element: XmlElement<'doc, 'input>,
) -> Option<String> {
    xml.length_attr(element, "w", LengthUsage::Dxa)
}

/// Select fixed table layout only for the exact fixed token.
pub(super) fn value_of_tbl_layout<'doc, 'input: 'doc>(
    xml: &'doc XmlParser<'input>,
    element: XmlElement<'doc, 'input>,
) -> &'static str {
    if xml.attr(element, "val") == Some("fixed") {
        "fixed"
    } else {
        "auto"
    }
}

/// Project a selected nonempty font theme into its CSS variable.
pub(super) fn theme_value<'doc, 'input: 'doc>(
    xml: &'doc XmlParser<'input>,
    element: XmlElement<'doc, 'input>,
    attr: &str,
) -> Option<String> {
    xml.attr(element, attr)
        .filter(|value| !value.is_empty())
        .map(|value| format!("var(--docx-{value}-font)"))
}
