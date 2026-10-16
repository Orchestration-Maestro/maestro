//! Borrowed XML theme-part preparation.
use super::super::{DocxError, XmlElement, XmlParser};
use super::drawing::{DmlTheme, parse_theme};
/// Prepared drawing theme.
#[derive(Debug, Default)]
pub struct ThemePart {
    /// Owned color and font schemes.
    pub theme: DmlTheme,
}
impl ThemePart {
    /// Prepare the theme's immediate schemes.
    ///
    /// # Errors
    /// Returns an XML-part error when the required theme-elements child is absent.
    pub fn parse_xml<'doc, 'input: 'doc>(
        xml: &'doc XmlParser<'input>,
        element: XmlElement<'doc, 'input>,
    ) -> Result<Self, DocxError> {
        parse_theme(xml, element).map(|theme| Self { theme })
    }
}
