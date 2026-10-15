//! In-place default-property dispatch over borrowed XML.
use super::super::{LengthUsage, XmlElement, XmlParser};
use super::values::{value_of_jc, value_of_size, value_of_text_alignment};
use super::xml_util::color_attr;
use indexmap::IndexMap;

/// Operation-local callback over the actual main style map.
type PropertyHandler<'handler, 'doc, 'input> =
    dyn FnMut(XmlElement<'doc, 'input>, &mut IndexMap<String, Option<String>>) -> bool + 'handler;

/// Per-call width policy and synchronous interception.
#[derive(Default)]
pub struct DefaultPropertiesOptions<'handler, 'doc, 'input: 'doc> {
    /// Suppress cell widths, leaving table widths unaffected.
    pub ignore_width: bool,
    /// Intercept each immediate child before dispatch, mutating the real main map.
    /// Returning true skips generic processing; the callback is never retained.
    pub handler: Option<&'handler mut PropertyHandler<'handler, 'doc, 'input>>,
}
/// Update supplied ordered styles from immediate property children.
/// The optional child map receives table borders and cell margins.
pub fn parse_default_properties<'doc, 'input: 'doc>(
    xml: &'doc XmlParser<'input>,
    element: XmlElement<'doc, 'input>,
    style: &mut IndexMap<String, Option<String>>,
    mut child_style: Option<&mut IndexMap<String, Option<String>>>,
    mut options: DefaultPropertiesOptions<'_, 'doc, 'input>,
) {
    let context = PropertyContext {
        xml,
        parent: element.local_name(),
        ignore_width: options.ignore_width,
    };
    for child in xml.elements(element, None) {
        if options
            .handler
            .as_mut()
            .is_some_and(|handler| handler(child, style))
        {
            continue;
        }
        match child.local_name() {
            "pBdr" | "tcBorders" => super::parse_border_properties(xml, child, style),
            "tblBorders" => super::parse_border_properties(
                xml,
                child,
                child_style.as_deref_mut().unwrap_or(style),
            ),
            "tblCellMar" | "tcMar" => {
                context.parse_margin_properties(child, child_style.as_deref_mut().unwrap_or(style));
            }
            _ => context.parse_property(child, style),
        }
    }
}
/// Borrowed operands used only during a property parse.
pub(super) struct PropertyContext<'doc, 'input: 'doc> {
    /// Shared XML queries.
    pub(super) xml: &'doc XmlParser<'input>,
    /// Parent's local name, selecting paragraph-only spacing.
    pub(super) parent: &'doc str,
    /// Cell-width suppression.
    pub(super) ignore_width: bool,
}
impl<'doc, 'input: 'doc> PropertyContext<'doc, 'input> {
    /// Dispatch one non-intercepted property into the main map.
    fn parse_property(
        &self,
        child: XmlElement<'doc, 'input>,
        style: &mut IndexMap<String, Option<String>>,
    ) {
        match child.local_name() {
            "trHeight" => self.parse_tr_height(child, style),
            "spacing" if self.parent == "pPr" => self.parse_spacing(child, style),
            "ind" | "tblInd" => self.parse_indentation(child, style),
            "rFonts" => self.parse_font(child, style),
            "u" => self.parse_underline(child, style),
            "sz" => {
                let size = self.xml.length_attr(child, "val", LengthUsage::FontSize);
                style.insert("min-height".into(), size.clone());
                style.insert("font-size".into(), size);
            }
            "tblCellSpacing" => {
                style.insert(
                    "border-spacing".into(),
                    super::values::value_of_margin(self.xml, child),
                );
                style.insert("border-collapse".into(), Some("separate".into()));
            }
            _ => {
                if let Some((key, value)) = self.direct_property(child) {
                    style.insert(key.into(), value);
                } else if let Some((key, value)) = self.boolean_style(child) {
                    style.insert(key.into(), Some(value.into()));
                }
            }
        }
    }
    /// Select direct nullable assignments.
    fn direct_property(
        &self,
        child: XmlElement<'doc, 'input>,
    ) -> Option<(&'static str, Option<String>)> {
        Some(match child.local_name() {
            "tblLayout" => (
                "table-layout",
                Some(super::values::value_of_tbl_layout(self.xml, child).into()),
            ),
            "bdr" => (
                "border",
                Some(super::values::value_of_border(self.xml, child)),
            ),
            "position" => (
                "vertical-align",
                self.xml.length_attr(child, "val", LengthUsage::FontSize),
            ),
            "lang" => ("$lang", self.xml.attr(child, "val").map(str::to_owned)),
            "color" => ("color", color_attr(self.xml, child, "val", "black")),
            "shd" => (
                "background-color",
                color_attr(self.xml, child, "fill", "inherit"),
            ),
            "highlight" => (
                "background-color",
                color_attr(self.xml, child, "val", "transparent"),
            ),
            "jc" => ("text-align", value_of_jc(self.xml, child)),
            "textAlignment" | "vAlign" => {
                ("vertical-align", value_of_text_alignment(self.xml, child))
            }
            "tcW" if self.ignore_width => return None,
            "tcW" | "tblW" => ("width", value_of_size(self.xml, child, "w")),
            _ => return None,
        })
    }
    /// Select exact boolean tokens with each property's literal fallback.
    fn boolean_style(
        &self,
        child: XmlElement<'doc, 'input>,
    ) -> Option<(&'static str, &'static str)> {
        let (key, on, off) = match child.local_name() {
            "strike" => ("text-decoration", "line-through", "none"),
            "b" => ("font-weight", "bold", "normal"),
            "i" => ("font-style", "italic", "normal"),
            "caps" => ("text-transform", "uppercase", "none"),
            "smallCaps" => ("font-variant", "small-caps", "none"),
            "suppressAutoHyphens" => ("hyphens", "none", "auto"),
            "vanish" | "rtl" | "bidi" | "wordWrap" => {
                let (key, value, fallback) = match child.local_name() {
                    "vanish" => ("display", "none", Some(true)),
                    "wordWrap" => ("overflow-wrap", "break-word", None),
                    _ => ("direction", "rtl", Some(true)),
                };
                return (self.xml.bool_attr(child, "val", fallback) == Some(true))
                    .then_some((key, value));
            }
            _ => return None,
        };
        Some((
            key,
            if self.xml.bool_attr(child, "val", Some(true)) == Some(true) {
                on
            } else {
                off
            },
        ))
    }
}
