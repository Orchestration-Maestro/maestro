//! Extended property part preparation.
use super::super::{XmlElement, XmlParser};
use super::{ExtendedPropsDeclaration, extended_props::parse_extended_props};
/// Prepared extended metadata.
#[derive(Debug, Default)]
pub struct ExtendedPropsPart {
    /// Selected property values.
    pub props: ExtendedPropsDeclaration,
}
impl ExtendedPropsPart {
    /// Prepare immediate extended properties.
    #[must_use]
    pub fn parse_xml<'doc, 'input: 'doc>(
        xml: &'doc XmlParser<'input>,
        root: XmlElement<'doc, 'input>,
    ) -> Self {
        Self {
            props: parse_extended_props(xml, root),
        }
    }
}
