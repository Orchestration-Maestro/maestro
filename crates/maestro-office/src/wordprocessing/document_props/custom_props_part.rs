//! Custom property part preparation.
use super::super::{DocxError, XmlElement, XmlParser};
use super::{CustomProperty, custom_props::parse_custom_props};
/// Prepared source-order custom properties.
#[derive(Debug, Default)]
pub struct CustomPropsPart {
    /// Values, retaining duplicate names.
    pub props: Vec<CustomProperty>,
}
impl CustomPropsPart {
    /// Prepare immediate custom properties.
    ///
    /// # Errors
    /// Returns an XML failure when a property has no value element.
    pub fn parse_xml<'doc, 'input: 'doc>(
        xml: &'doc XmlParser<'input>,
        root: XmlElement<'doc, 'input>,
    ) -> Result<Self, DocxError> {
        Ok(Self {
            props: parse_custom_props(xml, root)?,
        })
    }
}
