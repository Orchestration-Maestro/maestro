//! Ordered style definitions from a borrowed XML part.
use super::super::{DocxError, IDomStyle, XmlElement, XmlParser};
/// Prepared styles without inherited-style resolution.
#[derive(Debug, Default)]
pub struct StylesPart {
    /// Named and default styles in source order, retaining repeated identifiers.
    pub styles: Vec<IDomStyle>,
}
impl StylesPart {
    /// Prepare immediate named and default styles with the supplied cell-width policy.
    ///
    /// # Errors
    /// Returns an XML-part error for an aliases child lacking its value.
    pub fn parse_xml<'doc, 'input: 'doc>(
        xml: &'doc XmlParser<'input>,
        element: XmlElement<'doc, 'input>,
        ignore_width: bool,
    ) -> Result<Self, DocxError> {
        super::super::document_parser::styles::parse_styles_file(xml, element, ignore_width)
            .map(|styles| Self { styles })
    }
}
