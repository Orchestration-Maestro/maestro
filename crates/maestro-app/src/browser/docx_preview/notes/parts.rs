//! Note selection sharing one admitted part and the ordinary body owner.
use super::super::document_parser::{Framing, parse_selected};
use super::super::{DocxError, OwnedXmlElement, parse_xml_string};
use super::{WmlBaseNote, WmlEndnote, WmlFootnote};
/// Prepared footnotes in source order.
#[derive(Debug, Default)]
pub struct FootnotesPart {
    /// Immediate footnotes, retaining repeated identifiers.
    pub notes: Vec<WmlFootnote>,
}
impl FootnotesPart {
    /// Prepare immediate footnotes using the shared body parser.
    ///
    /// # Errors
    /// Propagates XML admission or selection failures.
    pub fn parse_xml(input: &OwnedXmlElement) -> Result<Self, DocxError> {
        parse_notes(input, "footnote").map(|notes| Self {
            notes: notes.into_iter().map(|base| WmlFootnote { base }).collect(),
        })
    }
}
/// Prepared endnotes in source order.
#[derive(Debug, Default)]
pub struct EndnotesPart {
    /// Immediate endnotes, retaining repeated identifiers.
    pub notes: Vec<WmlEndnote>,
}
impl EndnotesPart {
    /// Prepare immediate endnotes using the shared body parser.
    ///
    /// # Errors
    /// Propagates XML admission or selection failures.
    pub fn parse_xml(input: &OwnedXmlElement) -> Result<Self, DocxError> {
        parse_notes(input, "endnote").map(|notes| Self {
            notes: notes.into_iter().map(|base| WmlEndnote { base }).collect(),
        })
    }
}
/// Admit and select once, then construct independent arenas from borrowed notes.
fn parse_notes(input: &OwnedXmlElement, name: &str) -> Result<Vec<WmlBaseNote>, DocxError> {
    let xml = parse_xml_string(input.source(), false)?;
    let selected = xml.selected(input)?;
    Ok(xml
        .elements(selected, Some(name))
        .map(|note| WmlBaseNote {
            id: xml.attr(note, "id").map(str::to_owned),
            note_type: xml.attr(note, "type").map(str::to_owned),
            children: parse_selected(&xml, note, input, Framing::Body),
        })
        .collect())
}
