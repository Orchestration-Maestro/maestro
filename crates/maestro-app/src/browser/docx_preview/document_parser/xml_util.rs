//! Shared property color selection.
use super::super::{XmlElement, XmlParser};

/// Select a literal color before the theme fallback.
pub(super) fn color_attr<'doc, 'input: 'doc>(
    xml: &'doc XmlParser<'input>,
    element: XmlElement<'doc, 'input>,
    attr_name: &str,
) -> Option<String> {
    if let Some(value) = xml.attr(element, attr_name).filter(|s| !s.is_empty()) {
        return Some(match value {
            "auto" => "black".into(),
            "black" => "#000000".into(),
            "blue" => "#0000FF".into(),
            "cyan" => "#00FFFF".into(),
            "darkBlue" => "#000080".into(),
            "darkCyan" => "#008080".into(),
            "darkGray" => "#808080".into(),
            "darkGreen" => "#008000".into(),
            "darkMagenta" => "#800080".into(),
            "darkRed" => "#800000".into(),
            "darkYellow" => "#808000".into(),
            "green" => "#00FF00".into(),
            "lightGray" => "#C0C0C0".into(),
            "magenta" => "#FF00FF".into(),
            "none" => "transparent".into(),
            "red" => "#FF0000".into(),
            "white" => "#FFFFFF".into(),
            "yellow" => "#FFFF00".into(),
            _ => format!("#{value}"),
        });
    }
    xml.attr(element, "themeColor")
        .filter(|s| !s.is_empty())
        .map(|value| format!("var(--docx-{value}-color)"))
}
