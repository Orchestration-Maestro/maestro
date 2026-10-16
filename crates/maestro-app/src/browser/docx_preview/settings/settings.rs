//! Owned encountered Word settings and note defaults.
use super::super::{LengthUsage, XmlElement, XmlParser};
/// Encountered tab, note-default and hyphenation settings.
#[derive(Debug, Default)]
pub struct WmlSettings {
    /// Encountered nullable default tab size.
    pub default_tab_stop: Option<Option<String>>,
    /// Last encountered footnote-default group.
    pub footnote_props: Option<NoteProperties>,
    /// Last encountered endnote-default group.
    pub endnote_props: Option<NoteProperties>,
    /// Encountered nullable hyphenation flag.
    pub auto_hyphenation: Option<Option<bool>>,
}
/// Number format and ordered nullable default note identifiers.
#[derive(Debug, Default)]
pub struct NoteProperties {
    /// Last encountered nullable numbering-format value.
    pub nummering_format: Option<Option<String>>,
    /// Raw default identifiers in source order.
    pub default_note_ids: Vec<Option<String>>,
}
/// Replace encountered settings groups using shared attribute conversions.
pub(super) fn parse_settings(xml: &XmlParser<'_>, element: XmlElement<'_, '_>) -> WmlSettings {
    let mut result = WmlSettings::default();
    for child in xml.elements(element, None) {
        match child.local_name() {
            "defaultTabStop" => {
                result.default_tab_stop = Some(xml.length_attr(child, "val", LengthUsage::Dxa));
            }
            "footnotePr" => result.footnote_props = Some(parse_note_properties(xml, child)),
            "endnotePr" => result.endnote_props = Some(parse_note_properties(xml, child)),
            "autoHyphenation" => result.auto_hyphenation = Some(xml.bool_attr(child, "val", None)),
            _ => {}
        }
    }
    result
}
/// Read immediate format assignments and both default-note child kinds.
fn parse_note_properties(xml: &XmlParser<'_>, element: XmlElement<'_, '_>) -> NoteProperties {
    let mut result = NoteProperties::default();
    for child in xml.elements(element, None) {
        match child.local_name() {
            "numFmt" => result.nummering_format = Some(xml.attr(child, "val").map(str::to_owned)),
            "footnote" | "endnote" => result
                .default_note_ids
                .push(xml.attr(child, "id").map(str::to_owned)),
            _ => {}
        }
    }
    result
}
