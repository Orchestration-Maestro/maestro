//! Tables, rows and cells with their grids and property groups.
use super::super::{
    LengthUsage, WmlTable, WmlTableCell, WmlTableColumn, WmlTableRow, XmlElement, XmlParser,
};
use super::values::{add_size, class_name_of_cnf_style, class_name_of_tbl_look};
use super::{Data, DefaultPropertiesOptions, DocumentParser, DocxError, parse_default_properties};
use indexmap::IndexMap;

impl<'input> DocumentParser<'_, 'input> {
    /// Build a table from its rows, last grid and property groups.
    pub(super) fn parse_table(
        &mut self,
        element: XmlElement<'_, 'input>,
    ) -> Result<usize, DocxError> {
        let mut table = WmlTable::default();
        let mut children = Vec::new();
        for child in self.xml.elements(element, None) {
            match child.local_name() {
                "tr" => children.push(self.parse_table_row(child)?),
                "tblGrid" => table.columns = Some(parse_table_columns(self.xml, child)),
                "tblPr" => self.parse_table_properties(child, &mut table),
                _ => {}
            }
        }
        let id = self.push(Data::Table(Box::new(table)));
        self.tree.nodes[id].base.children = children;
        Ok(id)
    }
    /// Replace both style maps, keep earlier metadata, then turn alignment into margins.
    fn parse_table_properties(&self, element: XmlElement<'_, 'input>, table: &mut WmlTable) {
        let xml = self.xml;
        let mut style = IndexMap::new();
        let mut cell_style = IndexMap::new();
        let mut handler = |child: XmlElement<'_, 'input>,
                           style: &mut IndexMap<String, Option<String>>| {
            match child.local_name() {
                "tblStyle" => table.style_name = Some(xml.attr(child, "val").map(str::to_owned)),
                "tblLook" => table.class_name = Some(class_name_of_tbl_look(xml, child)),
                "tblpPr" => parse_table_position(xml, child, style),
                "tblStyleColBandSize" => {
                    table.col_band_size = Some(xml.int_attr(child, "val", None));
                }
                "tblStyleRowBandSize" => {
                    table.row_band_size = Some(xml.int_attr(child, "val", None));
                }
                "hidden" => {
                    style.insert("display".into(), Some("none".into()));
                }
                _ => return false,
            }
            true
        };
        parse_default_properties(
            xml,
            element,
            &mut style,
            Some(&mut cell_style),
            DefaultPropertiesOptions {
                ignore_width: self.ignore_width,
                handler: Some(&mut handler),
            },
        );
        let margins: &[&str] = match style.get("text-align").and_then(Option::as_deref) {
            Some("center") => &["margin-left", "margin-right"],
            Some("right") => &["margin-left"],
            _ => &[],
        };
        if !margins.is_empty() {
            style.shift_remove("text-align");
            for margin in margins {
                style.insert((*margin).into(), Some("auto".into()));
            }
        }
        table.css_style = Some(style);
        table.cell_style = Some(cell_style);
    }
    /// Build a row from its cells and property groups.
    fn parse_table_row(&mut self, element: XmlElement<'_, 'input>) -> Result<usize, DocxError> {
        let mut row = WmlTableRow::default();
        let mut children = Vec::new();
        for child in self.xml.elements(element, None) {
            match child.local_name() {
                "tc" => children.push(self.parse_table_cell(child)?),
                "trPr" | "tblPrEx" => self.parse_table_row_properties(child, &mut row),
                _ => {}
            }
        }
        let id = self.push(Data::Row(row));
        self.tree.nodes[id].base.children = children;
        Ok(id)
    }
    /// Replace row CSS while keeping earlier metadata.
    fn parse_table_row_properties(&self, element: XmlElement<'_, 'input>, row: &mut WmlTableRow) {
        let xml = self.xml;
        let mut style = IndexMap::new();
        let mut handler = |child: XmlElement<'_, 'input>,
                           _: &mut IndexMap<String, Option<String>>| {
            match child.local_name() {
                "cnfStyle" => row.class_name = Some(class_name_of_cnf_style(xml, child)),
                "tblHeader" => row.is_header = Some(xml.bool_attr(child, "val", None)),
                "gridBefore" => row.grid_before = Some(xml.int_attr(child, "val", None)),
                "gridAfter" => row.grid_after = Some(xml.int_attr(child, "val", None)),
                _ => return false,
            }
            true
        };
        parse_default_properties(
            xml,
            element,
            &mut style,
            None,
            DefaultPropertiesOptions {
                ignore_width: self.ignore_width,
                handler: Some(&mut handler),
            },
        );
        row.css_style = Some(style);
    }
    /// Build a cell from nested tables, paragraphs and property groups.
    fn parse_table_cell(&mut self, element: XmlElement<'_, 'input>) -> Result<usize, DocxError> {
        let mut cell = WmlTableCell::default();
        let mut children = Vec::new();
        for child in self.xml.elements(element, None) {
            match child.local_name() {
                "tbl" => children.push(self.parse_table(child)?),
                "p" => children.push(self.paragraph(child)?),
                "tcPr" => self.parse_table_cell_properties(child, &mut cell),
                _ => {}
            }
        }
        let id = self.push(Data::Cell(cell));
        self.tree.nodes[id].base.children = children;
        Ok(id)
    }
    /// Replace cell CSS while keeping earlier metadata, then apply text directions.
    fn parse_table_cell_properties(
        &self,
        element: XmlElement<'_, 'input>,
        cell: &mut WmlTableCell,
    ) {
        let xml = self.xml;
        let mut style = IndexMap::new();
        let mut handler = |child: XmlElement<'_, 'input>,
                           _: &mut IndexMap<String, Option<String>>| {
            match child.local_name() {
                "gridSpan" => cell.span = Some(xml.int_attr(child, "val", None)),
                "vMerge" => {
                    let merge = xml.attr(child, "val").unwrap_or("continue");
                    cell.vertical_merge = Some(merge.to_owned());
                }
                "cnfStyle" => cell.class_name = Some(class_name_of_cnf_style(xml, child)),
                _ => return false,
            }
            true
        };
        parse_default_properties(
            xml,
            element,
            &mut style,
            None,
            DefaultPropertiesOptions {
                ignore_width: self.ignore_width,
                handler: Some(&mut handler),
            },
        );
        parse_table_cell_vertical_text(xml, element, &mut style);
        cell.css_style = Some(style);
    }
}
/// Read immediate grid column widths in order.
fn parse_table_columns(xml: &XmlParser<'_>, element: XmlElement<'_, '_>) -> Vec<WmlTableColumn> {
    xml.elements(element, Some("gridCol"))
        .map(|column| WmlTableColumn {
            width: xml.length_attr(column, "w", LengthUsage::Dxa),
        })
        .collect()
}
/// Float the table and add each text distance to its current margin.
fn parse_table_position(
    xml: &XmlParser<'_>,
    element: XmlElement<'_, '_>,
    style: &mut IndexMap<String, Option<String>>,
) {
    style.insert("float".into(), Some("left".into()));
    for (key, attr) in [
        ("margin-bottom", "bottomFromText"),
        ("margin-left", "leftFromText"),
        ("margin-right", "rightFromText"),
        ("margin-top", "topFromText"),
    ] {
        let margin = style.entry(key.into()).or_default();
        *margin = add_size(
            margin.take(),
            xml.length_attr(element, attr, LengthUsage::Dxa),
        );
    }
}
/// Apply each text direction; unmapped values select horizontal text without a transform.
fn parse_table_cell_vertical_text(
    xml: &XmlParser<'_>,
    element: XmlElement<'_, '_>,
    style: &mut IndexMap<String, Option<String>>,
) {
    for child in xml.elements(element, Some("textDirection")) {
        let (writing_mode, transform) = match xml.attr(child, "val") {
            Some("btLr") => ("vertical-rl", Some("rotate(180deg)")),
            Some("lrTb") => ("vertical-lr", Some("none")),
            Some("tbRl") => ("vertical-rl", Some("none")),
            _ => ("horizontal-tb", None),
        };
        style.insert("writing-mode".into(), Some(writing_mode.into()));
        style.insert("transform".into(), transform.map(Into::into));
    }
}
