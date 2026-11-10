//! Raw numbering part with its prepared concrete view.
use super::super::{IDomNumbering, XmlElement, XmlParser};
use super::{AbstractNumbering, Numbering, NumberingBulletPicture};
/// Owned numbering part contents.
#[derive(Debug, Default)]
pub struct NumberingPart {
    /// Concrete references in source order.
    pub numberings: Vec<Numbering>,
    /// Definitions in source order.
    pub abstract_numberings: Vec<AbstractNumbering>,
    /// Picture slots, including incomplete chains.
    pub bullet_pictures: Vec<Option<NumberingBulletPicture>>,
    /// Resolved concrete levels.
    pub dom_numberings: Vec<IDomNumbering>,
}
impl NumberingPart {
    /// Parse raw records and prepare their concrete levels from one borrowed selection.
    #[must_use]
    pub fn parse_xml<'doc, 'input: 'doc>(
        xml: &'doc XmlParser<'input>,
        root: XmlElement<'doc, 'input>,
        ignore_width: bool,
    ) -> Self {
        let mut parsed = super::parser::Reader { xml, ignore_width }.parse(root);
        parsed.part.dom_numberings = super::super::document_parser::numbering::prepare(&parsed);
        parsed.part
    }
}
