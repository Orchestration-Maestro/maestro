//! Border shorthand values.
use super::super::{LengthUsage, VerticalAlign, XmlElement, XmlParser};
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

/// Select exact fixed layout from type, falling back to val only when absent.
pub(super) fn value_of_tbl_layout<'doc, 'input: 'doc>(
    xml: &'doc XmlParser<'input>,
    element: XmlElement<'doc, 'input>,
) -> &'static str {
    if xml
        .attr(element, "type")
        .or_else(|| xml.attr(element, "val"))
        == Some("fixed")
    {
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

/// Select conditional classes from an authoritative mask or the document flags.
pub(super) fn class_name_of_cnf_style(xml: &XmlParser<'_>, element: XmlElement<'_, '_>) -> String {
    let classes = [
        ("first-row", "firstRow"),
        ("last-row", "lastRow"),
        ("first-col", "firstColumn"),
        ("last-col", "lastColumn"),
        ("odd-col", "oddVBand"),
        ("even-col", "evenVBand"),
        ("odd-row", "oddHBand"),
        ("even-row", "evenHBand"),
        ("ne-cell", "firstRowLastColumn"),
        ("nw-cell", "firstRowFirstColumn"),
        ("se-cell", "lastRowLastColumn"),
        ("sw-cell", "lastRowFirstColumn"),
    ];
    let mask = xml.attr(element, "val");
    classes
        .iter()
        .enumerate()
        .filter(|(index, (_, flag))| {
            mask.map_or_else(
                || xml.bool_attr(element, flag, Some(false)) == Some(true),
                |value| value.as_bytes().get(*index) == Some(&b'1'),
            )
        })
        .map(|(_, (class, _))| *class)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Select the supported run alignment tag, leaving other values null.
pub(super) fn value_of_vert_align(
    xml: &XmlParser<'_>,
    element: XmlElement<'_, '_>,
) -> Option<VerticalAlign> {
    match xml.attr(element, "val") {
        Some("subscript") => Some(VerticalAlign::Sub),
        Some("superscript") => Some(VerticalAlign::Sup),
        _ => None,
    }
}
