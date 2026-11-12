//! Core property part preparation.
use super::super::{XmlElement, XmlParser};
use super::{CorePropsDeclaration, core_props::parse_core_props};
/// Prepared core metadata.
#[derive(Debug, Default)]
pub struct CorePropsPart {
    /// Selected property values.
    pub props: CorePropsDeclaration,
}
impl CorePropsPart {
    /// Prepare immediate core properties.
    #[must_use]
    pub fn parse_xml<'doc, 'input: 'doc>(
        xml: &'doc XmlParser<'input>,
        root: XmlElement<'doc, 'input>,
    ) -> Self {
        Self {
            props: parse_core_props(xml, root),
        }
    }
}
