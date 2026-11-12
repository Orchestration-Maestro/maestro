//! Comment selection reusing one admitted source and the ordinary body owner.
use super::super::{DocxError, OwnedXmlElement, WmlComment, parse_xml_string};
use super::{Framing, parse_selected};
/// Admit and select once, then prepare each immediate comment's independent arena.
pub(crate) fn parse_comments(
    input: &OwnedXmlElement,
    ignore_width: bool,
) -> Result<Vec<WmlComment>, DocxError> {
    let xml = parse_xml_string(input.source(), false)?;
    let root = xml.selected(input)?;
    xml.elements(root, Some("comment"))
        .map(|comment| {
            Ok(WmlComment {
                id: xml.attr(comment, "id").map(str::to_owned),
                author: xml.attr(comment, "author").map(str::to_owned),
                initials: xml.attr(comment, "initials").map(str::to_owned),
                date: xml.attr(comment, "date").map(str::to_owned),
                children: parse_selected(&xml, comment, Framing::Body, ignore_width)?,
                css_style: indexmap::IndexMap::new(),
            })
        })
        .collect()
}
