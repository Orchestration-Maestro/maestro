//! Owned run formatting metadata.
use super::super::{XmlElement, XmlParser};
use super::common::{CommonProperties, parse_common_property};
/// Owned run properties; outer options record property encounters.
#[derive(Debug, Default)]
pub struct RunProperties {
    /// Fresh ordered CSS from the last property group.
    pub css_style: Option<indexmap::IndexMap<String, Option<String>>>,
    /// Encountered run style reference.
    pub style_name: Option<Option<String>>,
    /// Encountered tag alignment, including unsupported values.
    pub vertical_align: Option<Option<VerticalAlign>>,
}
/// Supported run alignment tag choices.
#[derive(Debug)]
pub enum VerticalAlign {
    /// Subscript tag.
    Sub,
    /// Superscript tag.
    Sup,
}

/// Read common metadata only from a nested paragraph run-property group.
pub(crate) fn parse_run_properties(
    xml: &XmlParser<'_>,
    element: XmlElement<'_, '_>,
) -> CommonProperties {
    let mut result = CommonProperties::default();
    for child in xml.elements(element, None) {
        parse_common_property(xml, child, &mut result);
    }
    result
}
