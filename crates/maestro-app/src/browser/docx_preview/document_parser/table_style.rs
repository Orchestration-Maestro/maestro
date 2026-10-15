//! Owned conditional table-style preparation.
use super::super::{XmlElement, XmlParser};
use super::default_properties::{DefaultPropertiesOptions, parse_default_properties};
use indexmap::IndexMap;

/// One conditional selector with its owned ordered CSS values.
pub struct TableStyle {
    /// CSS target selector.
    pub target: String,
    /// Conditional region modifier.
    pub r#mod: String,
    /// Separately owned nullable CSS values.
    pub values: IndexMap<String, Option<String>>,
}
/// Prepare supported conditional regions, retaining property-child order.
#[must_use]
pub fn parse_table_style<'doc, 'input: 'doc>(
    xml: &'doc XmlParser<'input>,
    element: XmlElement<'doc, 'input>,
    ignore_width: bool,
) -> Vec<TableStyle> {
    let Some((modifier, selector)) = table_region(xml.attr(element, "type")) else {
        return Vec::new();
    };
    xml.elements(element, None)
        .filter_map(|child| {
            let target = match child.local_name() {
                "pPr" => format!("{selector} p"),
                "rPr" => format!("{selector} span"),
                "tblPr" | "tcPr" => selector.into(),
                _ => return None,
            };
            let mut values = IndexMap::new();
            parse_default_properties(
                xml,
                child,
                &mut values,
                None,
                DefaultPropertiesOptions {
                    ignore_width,
                    handler: None,
                },
            );
            Some(TableStyle {
                target,
                r#mod: modifier.into(),
                values,
            })
        })
        .collect()
}
/// Select the modifier and base target for supported table regions.
fn table_region(region: Option<&str>) -> Option<(&'static str, &'static str)> {
    Some(match region? {
        "firstRow" => (".first-row", "tr.first-row td"),
        "lastRow" => (".last-row", "tr.last-row td"),
        "firstCol" => (".first-col", "td.first-col"),
        "lastCol" => (".last-col", "td.last-col"),
        "band1Vert" => (":not(.no-vband)", "td.odd-col"),
        "band2Vert" => (":not(.no-vband)", "td.even-col"),
        "band1Horz" => (":not(.no-hband)", "tr.odd-row"),
        "band2Horz" => (":not(.no-hband)", "tr.even-row"),
        _ => return None,
    })
}
