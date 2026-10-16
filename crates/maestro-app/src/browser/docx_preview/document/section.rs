//! Owned section records and immediate property readers.
use super::super::{XmlElement, XmlParser};
use super::border::{Borders, parse_borders};
use super::common::LengthUsage;

/// Page dimensions and open orientation spelling.
#[derive(Clone, Debug, Default)]
pub struct PageSize {
    /// Converted page width.
    pub width: Option<String>,
    /// Converted page height.
    pub height: Option<String>,
    /// Raw orientation attribute.
    pub orientation: Option<String>,
}
/// Page margins and header/footer offsets.
#[derive(Clone, Debug, Default)]
pub struct PageMargins {
    /// Converted left margin.
    pub left: Option<String>,
    /// Converted right margin.
    pub right: Option<String>,
    /// Converted top margin.
    pub top: Option<String>,
    /// Converted bottom margin.
    pub bottom: Option<String>,
    /// Converted header offset.
    pub header: Option<String>,
    /// Converted footer offset.
    pub footer: Option<String>,
    /// Converted gutter margin.
    pub gutter: Option<String>,
}
/// One column's dimensions.
#[derive(Clone, Debug, Default)]
pub struct Column {
    /// Converted column width.
    pub width: Option<String>,
    /// Converted spacing after the column.
    pub space: Option<String>,
}
/// Column layout attributes and ordered explicit columns.
#[derive(Clone, Debug, Default)]
pub struct Columns {
    /// Numeric column count.
    pub number_of_columns: Option<f64>,
    /// Converted default column spacing.
    pub space: Option<String>,
    /// Nullable separator flag.
    pub separator: Option<bool>,
    /// Equal-width flag; the parser defaults missing or unknown tokens to true.
    pub equal_width: bool,
    /// Immediate column records in encounter order.
    pub columns: Vec<Column>,
}
/// Page-number attributes.
#[derive(Clone, Debug, Default)]
pub struct PageNumber {
    /// Raw chapter separator.
    pub chap_sep: Option<String>,
    /// Raw chapter style.
    pub chap_style: Option<String>,
    /// Open numbering format spelling.
    pub format: Option<String>,
    /// Numeric starting page number.
    pub start: Option<f64>,
}
/// Raw relationship reference to a footer or header.
#[derive(Clone, Debug, Default)]
pub struct FooterHeaderReference {
    /// Relationship identifier.
    pub id: Option<String>,
    /// Open reference kind spelling.
    pub r#type: Option<String>,
}
/// Encountered section groups with owned attribute values.
#[derive(Clone, Debug, Default)]
pub struct SectionProperties {
    /// Last encountered page dimensions.
    pub page_size: Option<PageSize>,
    /// Encountered section type with a nullable attribute.
    pub r#type: Option<Option<String>>,
    /// Last encountered page margins.
    pub page_margins: Option<PageMargins>,
    /// Last encountered column layout.
    pub columns: Option<Columns>,
    /// Header references in encounter order, absent before the first reference.
    pub header_refs: Option<Vec<FooterHeaderReference>>,
    /// Footer references in encounter order, absent before the first reference.
    pub footer_refs: Option<Vec<FooterHeaderReference>>,
    /// Last encountered title-page flag.
    pub title_page: Option<bool>,
    /// Last encountered raw page borders.
    pub page_borders: Option<Borders>,
    /// Last encountered page-number attributes.
    pub page_number: Option<PageNumber>,
}

/// Read immediate section children, replacing repeated singleton groups.
pub(crate) fn parse_section_properties(
    xml: &XmlParser<'_>,
    element: XmlElement<'_, '_>,
) -> SectionProperties {
    let mut section = SectionProperties::default();
    for child in xml.elements(element, None) {
        match child.local_name() {
            "pgSz" => section.page_size = Some(parse_page_size(xml, child)),
            "pgMar" => section.page_margins = Some(parse_page_margins(xml, child)),
            "type" => section.r#type = Some(xml.attr(child, "val").map(str::to_owned)),
            "cols" => section.columns = Some(parse_columns(xml, child)),
            "headerReference" => section
                .header_refs
                .get_or_insert_default()
                .push(parse_footer_header_reference(xml, child)),
            "footerReference" => section
                .footer_refs
                .get_or_insert_default()
                .push(parse_footer_header_reference(xml, child)),
            "titlePg" => section.title_page = xml.bool_attr(child, "val", Some(true)),
            "pgBorders" => section.page_borders = Some(parse_borders(xml, child)),
            "pgNumType" => section.page_number = Some(parse_page_number(xml, child)),
            _ => {}
        }
    }
    section
}
/// Read page dimensions independently through the shared length owner.
fn parse_page_size(xml: &XmlParser<'_>, element: XmlElement<'_, '_>) -> PageSize {
    PageSize {
        width: xml.length_attr(element, "w", LengthUsage::Dxa),
        height: xml.length_attr(element, "h", LengthUsage::Dxa),
        orientation: xml.attr(element, "orient").map(str::to_owned),
    }
}
/// Read each margin and offset through the shared length owner.
fn parse_page_margins(xml: &XmlParser<'_>, element: XmlElement<'_, '_>) -> PageMargins {
    PageMargins {
        left: xml.length_attr(element, "left", LengthUsage::Dxa),
        right: xml.length_attr(element, "right", LengthUsage::Dxa),
        top: xml.length_attr(element, "top", LengthUsage::Dxa),
        bottom: xml.length_attr(element, "bottom", LengthUsage::Dxa),
        header: xml.length_attr(element, "header", LengthUsage::Dxa),
        footer: xml.length_attr(element, "footer", LengthUsage::Dxa),
        gutter: xml.length_attr(element, "gutter", LengthUsage::Dxa),
    }
}

/// Read column defaults and ordered immediate column records.
fn parse_columns(xml: &XmlParser<'_>, element: XmlElement<'_, '_>) -> Columns {
    Columns {
        number_of_columns: xml.int_attr(element, "num", None),
        space: xml.length_attr(element, "space", LengthUsage::Dxa),
        separator: xml.bool_attr(element, "sep", None),
        equal_width: xml.bool_attr(element, "equalWidth", Some(true)) == Some(true),
        columns: xml
            .elements(element, Some("col"))
            .map(|child| Column {
                width: xml.length_attr(child, "w", LengthUsage::Dxa),
                space: xml.length_attr(child, "space", LengthUsage::Dxa),
            })
            .collect(),
    }
}
/// Read raw page-number strings alongside the shared numeric conversion.
fn parse_page_number(xml: &XmlParser<'_>, element: XmlElement<'_, '_>) -> PageNumber {
    PageNumber {
        chap_sep: xml.attr(element, "chapSep").map(str::to_owned),
        chap_style: xml.attr(element, "chapStyle").map(str::to_owned),
        format: xml.attr(element, "fmt").map(str::to_owned),
        start: xml.int_attr(element, "start", None),
    }
}
/// Read a relationship identifier and its open reference kind.
fn parse_footer_header_reference(
    xml: &XmlParser<'_>,
    element: XmlElement<'_, '_>,
) -> FooterHeaderReference {
    FooterHeaderReference {
        id: xml.attr(element, "id").map(str::to_owned),
        r#type: xml.attr(element, "type").map(str::to_owned),
    }
}
