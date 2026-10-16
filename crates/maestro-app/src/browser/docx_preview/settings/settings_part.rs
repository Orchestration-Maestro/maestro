//! Borrowed XML settings-part preparation.
use super::super::{XmlElement, XmlParser};
use super::properties::{WmlSettings, parse_settings};
/// Prepared document settings.
#[derive(Debug, Default)]
pub struct SettingsPart {
    /// Owned encountered settings.
    pub settings: WmlSettings,
}
impl SettingsPart {
    /// Prepare settings from an admitted XML selection.
    #[must_use]
    pub fn parse_xml<'doc, 'input: 'doc>(
        xml: &'doc XmlParser<'input>,
        element: XmlElement<'doc, 'input>,
    ) -> Self {
        Self {
            settings: parse_settings(xml, element),
        }
    }
}
