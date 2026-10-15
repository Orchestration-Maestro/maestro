//! Specialized CSS formatting from immediate Word property nodes.
use super::super::{LengthUsage, XmlElement};
use super::default_properties::PropertyContext;
use super::values::value_of_margin;
use super::xml_util::color_attr;
use indexmap::IndexMap;

impl<'doc, 'input: 'doc> PropertyContext<'doc, 'input> {
    /// Write immediate physical-side margins, retaining absent values.
    pub(super) fn parse_margin_properties(
        &self,
        node: XmlElement<'doc, 'input>,
        output: &mut IndexMap<String, Option<String>>,
    ) {
        for child in self.xml.elements(node, None) {
            let key = match child.local_name() {
                "left" => "padding-left",
                "right" => "padding-right",
                "top" => "padding-top",
                "bottom" => "padding-bottom",
                _ => continue,
            };
            output.insert(key.into(), value_of_margin(self.xml, child));
        }
    }

    /// Select underline decoration, returning early only for an absent token.
    pub(super) fn parse_underline(
        &self,
        node: XmlElement<'doc, 'input>,
        style: &mut IndexMap<String, Option<String>>,
    ) {
        let Some(value) = self.xml.attr(node, "val") else {
            return;
        };
        let decoration = match value {
            "dash" | "dashDotDotHeavy" | "dashDotHeavy" | "dashedHeavy" | "dashLong"
            | "dashLongHeavy" | "dotDash" | "dotDotDash" => Some("underline dashed"),
            "dotted" | "dottedHeavy" => Some("underline dotted"),
            "double" => Some("underline double"),
            "single" | "thick" | "words" => Some("underline"),
            "wave" | "wavyDouble" | "wavyHeavy" => Some("underline wavy"),
            "none" => Some("none"),
            _ => None,
        };
        if let Some(decoration) = decoration {
            style.insert("text-decoration".into(), Some(decoration.into()));
        }
        if let Some(color) = color_attr(self.xml, node, "color", "black") {
            style.insert("text-decoration-color".into(), Some(color));
        }
    }

    /// Enclose selected font families, then deduplicate rendered strings in order.
    pub(super) fn parse_font(
        &self,
        node: XmlElement<'doc, 'input>,
        style: &mut IndexMap<String, Option<String>>,
    ) {
        let theme = super::values::theme_value(self.xml, node, "asciiTheme");
        let fonts: indexmap::IndexSet<_> = [
            self.xml.attr(node, "ascii"),
            theme.as_deref(),
            self.xml.attr(node, "eastAsia"),
        ]
        .into_iter()
        .flatten()
        .filter(|value| !value.is_empty())
        .map(super::super::utils::enclose_font_family)
        .collect();
        if !fonts.is_empty() {
            style.insert(
                "font-family".into(),
                Some(fonts.into_iter().collect::<Vec<_>>().join(", ")),
            );
        }
    }

    /// Apply hanging after first-line indentation and prefer physical sides.
    pub(super) fn parse_indentation(
        &self,
        node: XmlElement<'doc, 'input>,
        style: &mut IndexMap<String, Option<String>>,
    ) {
        let length = |attr| self.xml.length_attr(node, attr, LengthUsage::Dxa);
        if let Some(value) = length("firstLine") {
            style.insert("text-indent".into(), Some(value));
        }
        if let Some(value) = length("hanging") {
            style.insert("text-indent".into(), Some(format!("-{value}")));
        }
        for (physical, logical, key) in [
            ("left", "start", "margin-inline-start"),
            ("right", "end", "margin-inline-end"),
        ] {
            if let Some(value) = length(physical).or_else(|| length(logical)) {
                style.insert(key.into(), Some(value));
            }
        }
    }

    /// Apply paragraph margins and line-rule-specific height assignments.
    pub(super) fn parse_spacing(
        &self,
        node: XmlElement<'doc, 'input>,
        style: &mut IndexMap<String, Option<String>>,
    ) {
        for (attr, key) in [("before", "margin-top"), ("after", "margin-bottom")] {
            if let Some(value) = self.xml.length_attr(node, attr, LengthUsage::Dxa) {
                style.insert(key.into(), Some(value));
            }
        }
        let Some(line) = self.xml.int_attr(node, "line", None) else {
            return;
        };
        let value = match self.xml.attr(node, "lineRule") {
            Some("auto") => ryu_js::Buffer::new()
                .format_to_fixed(line / 240.0, 2)
                .to_owned(),
            Some("atLeast") => format!(
                "calc(100% + {}pt)",
                ryu_js::Buffer::new().format(line / 20.0)
            ),
            _ => {
                let value = format!("{}pt", ryu_js::Buffer::new().format(line / 20.0));
                style.insert("min-height".into(), Some(value.clone()));
                value
            }
        };
        style.insert("line-height".into(), Some(value));
    }

    /// All row-height rules share the same nullable height assignment.
    pub(super) fn parse_tr_height(
        &self,
        node: XmlElement<'doc, 'input>,
        output: &mut IndexMap<String, Option<String>>,
    ) {
        output.insert(
            "height".into(),
            self.xml.length_attr(node, "val", LengthUsage::Dxa),
        );
    }
}
