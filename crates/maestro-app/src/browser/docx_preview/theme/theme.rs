//! Owned drawing color and typeface schemes.
use super::super::{DocxError, XmlElement, XmlParser};
/// Optional theme color and font schemes.
#[derive(Debug, Default)]
pub struct DmlTheme {
    /// Last encountered color scheme.
    pub color_scheme: Option<DmlColorScheme>,
    /// Last encountered font scheme.
    pub font_scheme: Option<DmlFontScheme>,
}
/// Ordered raw color slots, before CSS conversion.
#[derive(Debug, Default)]
pub struct DmlColorScheme {
    /// Nullable authored scheme name.
    pub name: Option<String>,
    /// Slot values in first-insertion order.
    pub colors: indexmap::IndexMap<String, Option<String>>,
}
/// Independent major and minor font groups.
#[derive(Debug, Default)]
pub struct DmlFontScheme {
    /// Nullable authored scheme name.
    pub name: Option<String>,
    /// Last encountered major font group.
    pub major_font: Option<DmlFormInfo>,
    /// Last encountered minor font group.
    pub minor_font: Option<DmlFormInfo>,
}
/// Encountered typeface children with nullable attributes.
#[derive(Debug, Default)]
pub struct DmlFormInfo {
    /// Latin child and its nullable typeface.
    pub latin_typeface: Option<Option<String>>,
    /// East Asian child and its nullable typeface.
    pub ea_typeface: Option<Option<String>>,
    /// Complex-script child and its nullable typeface.
    pub cs_typeface: Option<Option<String>>,
}
/// Prepare schemes from the first required theme-elements child.
pub(super) fn parse_theme(
    xml: &XmlParser<'_>,
    element: XmlElement<'_, '_>,
) -> Result<DmlTheme, DocxError> {
    let elements = xml
        .element(element, "themeElements")
        .ok_or_else(|| DocxError::Xml {
            path: None,
            message: "Theme elements are missing".into(),
        })?;
    let mut result = DmlTheme::default();
    for child in xml.elements(elements, None) {
        match child.local_name() {
            "clrScheme" => result.color_scheme = Some(parse_color_scheme(xml, child)),
            "fontScheme" => result.font_scheme = Some(parse_font_scheme(xml, child)),
            _ => {}
        }
    }
    Ok(result)
}
/// Select direct sRGB before system-color fallback, retaining replacement order.
fn parse_color_scheme(xml: &XmlParser<'_>, element: XmlElement<'_, '_>) -> DmlColorScheme {
    let mut result = DmlColorScheme {
        name: xml.attr(element, "name").map(str::to_owned),
        ..Default::default()
    };
    for child in xml.elements(element, None) {
        let selected = xml
            .element(child, "srgbClr")
            .map(|e| (e, "val"))
            .or_else(|| xml.element(child, "sysClr").map(|e| (e, "lastClr")));
        if let Some((color, attr)) = selected {
            result.colors.insert(
                child.local_name().into(),
                xml.attr(color, attr).map(str::to_owned),
            );
        }
    }
    result
}
/// Replace each encountered font group independently.
fn parse_font_scheme(xml: &XmlParser<'_>, element: XmlElement<'_, '_>) -> DmlFontScheme {
    let mut result = DmlFontScheme {
        name: xml.attr(element, "name").map(str::to_owned),
        ..Default::default()
    };
    for child in xml.elements(element, None) {
        match child.local_name() {
            "majorFont" => result.major_font = Some(parse_font_info(xml, child)),
            "minorFont" => result.minor_font = Some(parse_font_info(xml, child)),
            _ => {}
        }
    }
    result
}
/// Keep child absence separate from a missing typeface attribute.
fn parse_font_info(xml: &XmlParser<'_>, element: XmlElement<'_, '_>) -> DmlFormInfo {
    let typeface = |name| {
        xml.element(element, name)
            .map(|child| xml.attr(child, "typeface").map(str::to_owned))
    };
    DmlFormInfo {
        latin_typeface: typeface("latin"),
        ea_typeface: typeface("ea"),
        cs_typeface: typeface("cs"),
    }
}
