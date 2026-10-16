//! Bookmark markers retaining authored identifiers and column numbers.
use super::super::{OpenXmlElementData as Data, XmlElement, XmlParser};

/// Read a start marker with independent nullable columns.
pub(crate) fn parse_bookmark_start(xml: &XmlParser<'_>, element: XmlElement<'_, '_>) -> Data {
    Data::BookmarkStart {
        id: xml.attr(element, "id").map(str::to_owned),
        name: xml.attr(element, "name").map(str::to_owned),
        col_first: xml.int_attr(element, "colFirst", None),
        col_last: xml.int_attr(element, "colLast", None),
    }
}
/// Read an end marker without pairing or deduplicating identifiers.
pub(crate) fn parse_bookmark_end(xml: &XmlParser<'_>, element: XmlElement<'_, '_>) -> Data {
    Data::BookmarkEnd {
        id: xml.attr(element, "id").map(str::to_owned),
    }
}
