//! Tables, their column groups, rows and cells, and vertical merges.
use super::{HtmlRenderer, render_style_values};
use crate::docx_preview::html_element::HtmlElement;
use maestro_app::browser::docx_preview::{
    OpenXmlElementData as Data, OpenXmlElementTree, WmlTable, WmlTableCell, WmlTableColumn,
    WmlTableRow,
};
use num_traits::ToPrimitive;
use std::collections::HashMap;

/// One open table: finished rows, the row being filled and its merge state.
///
/// A nested table opens its own state, so the enclosing table's state is restored when the
/// nested one ends.
struct Table {
    /// The table with its finished rows.
    element: HtmlElement,
    /// The row being filled.
    row: HtmlElement,
    /// Grid column of the next cell in the row.
    col: usize,
    /// Running vertical merge by grid column, as an index into `spans`.
    vertical_merge: HashMap<usize, usize>,
    /// Merge-starting cells and the rows they span.
    spans: Vec<RowSpan>,
}

/// A merge-starting cell's position and its row count.
struct RowSpan {
    /// Position of its row among the table's children.
    row: usize,
    /// Position of the cell among its row's children.
    cell: usize,
    /// Rows the cell spans.
    count: u32,
}

impl<'a> HtmlRenderer<'a> {
    /// Render a table with its column group, rows, class and declarations.
    pub(super) fn render_table(
        &mut self,
        tree: &'a OpenXmlElementTree,
        index: usize,
        record: &'a WmlTable,
    ) -> HtmlElement {
        let mut table = Table {
            element: HtmlElement::new("table"),
            row: HtmlElement::new("tr"),
            col: 0,
            vertical_merge: HashMap::new(),
            spans: Vec::new(),
        };
        if let Some(columns) = &record.columns {
            table.element.push_element(render_table_columns(columns));
        }
        let style_name = record.style_name.as_ref().and_then(Option::as_deref);
        self.render_class(record.class_name.as_deref(), style_name, &mut table.element);
        for &row in &tree.nodes[index].base.children {
            if let Data::Row(row_record) = &tree.nodes[row].data {
                self.render_table_row(tree, row, row_record, &mut table);
            }
        }
        let Table {
            mut element, spans, ..
        } = table;
        render_style_values(record.css_style.as_ref(), &mut element);
        for span in spans {
            if let Some(cell) = element
                .child_mut(span.row)
                .and_then(|row| row.child_mut(span.cell))
            {
                cell.set_attribute("rowspan", span.count.to_string());
            }
        }
        element
    }

    /// Render a row between its leading and trailing grid placeholders.
    fn render_table_row(
        &mut self,
        tree: &'a OpenXmlElementTree,
        index: usize,
        record: &'a WmlTableRow,
        table: &mut Table,
    ) {
        table.col = 0;
        if let Some(count) = column_count(record.grid_before.flatten()) {
            table.row.push_element(render_table_cell_placeholder(count));
        }
        self.render_class(record.class_name.as_deref(), None, &mut table.row);
        for &cell in &tree.nodes[index].base.children {
            if let Data::Cell(cell_record) = &tree.nodes[cell].data {
                self.render_table_cell(tree, cell, cell_record, table);
            }
        }
        render_style_values(record.css_style.as_ref(), &mut table.row);
        if let Some(count) = column_count(record.grid_after.flatten()) {
            table.row.push_element(render_table_cell_placeholder(count));
        }
        let row = std::mem::replace(&mut table.row, HtmlElement::new("tr"));
        table.element.push_element(row);
    }

    /// Render a cell, starting, extending or ending its column's vertical merge.
    fn render_table_cell(
        &mut self,
        tree: &'a OpenXmlElementTree,
        index: usize,
        record: &'a WmlTableCell,
        table: &mut Table,
    ) {
        let mut cell = self.render_container(tree, index, "td");
        let key = table.col;
        match record
            .vertical_merge
            .as_deref()
            .filter(|merge| !merge.is_empty())
        {
            Some("restart") => {
                table.vertical_merge.insert(key, table.spans.len());
                table.spans.push(RowSpan {
                    row: table.element.children.len(),
                    cell: table.row.children.len(),
                    count: 1,
                });
            }
            Some(_) => {
                if let Some(span) = table
                    .vertical_merge
                    .get(&key)
                    .and_then(|&span| table.spans.get_mut(span))
                {
                    span.count += 1;
                    cell.set_style("display", Some("none"));
                }
            }
            None => {
                table.vertical_merge.remove(&key);
            }
        }
        self.render_class(record.class_name.as_deref(), None, &mut cell);
        render_style_values(record.css_style.as_ref(), &mut cell);
        let span = column_count(record.span.flatten());
        if let Some(span) = span {
            cell.set_attribute("colspan", span.to_string());
        }
        table.col += span.unwrap_or(1);
        table.row.push_element(cell);
    }
}

/// Render the column group, giving each column its width when it has one.
fn render_table_columns(columns: &[WmlTableColumn]) -> HtmlElement {
    let mut group = HtmlElement::new("colgroup");
    for column in columns {
        let mut element = HtmlElement::new("col");
        if let Some(width) = column.width.as_deref().filter(|width| !width.is_empty()) {
            element.set_style("width", Some(width));
        }
        group.push_element(element);
    }
    group
}

/// Render a borderless cell spanning grid columns the row leaves empty.
fn render_table_cell_placeholder(count: usize) -> HtmlElement {
    let mut element = HtmlElement::new("td");
    element.set_attribute("colspan", count.to_string());
    element.set_style("border", Some("none"));
    element
}

/// Read a span or grid offset as a cell's column count; zero and NaN read as none, and a
/// cell spans between 1 and 1000 columns.
fn column_count(value: Option<f64>) -> Option<usize> {
    let value = value.filter(|value| *value != 0.0 && !value.is_nan())?;
    value.trunc().clamp(1.0, 1000.0).to_usize()
}
