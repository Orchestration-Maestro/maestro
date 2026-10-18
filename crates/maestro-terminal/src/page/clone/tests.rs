//! Source-row management selects placeholder handling independently of cell fields.

use super::{Cell, CellCopy, CellPosition, Page};

#[test]
fn copied_placeholder_uses_source_row_management() {
    let mut page = Page::init(crate::page::layout::Capacity {
        cols: 1,
        rows: 1,
        ..Default::default()
    });
    let data = CellCopy {
        cell: Cell::init('\u{10eeee}'),
        suffix: None,
        style: None,
        link: None,
        managed: true,
    };
    page.copy_cell(data, CellPosition { x: 0, y: 0 }).unwrap();
    assert!(page.get_row(0).kitty_virtual_placeholder());
}

#[test]
fn copied_style_marks_row_before_admission_failure() {
    let mut page = Page::init(crate::page::layout::Capacity {
        cols: 2,
        rows: 1,
        styles: 0,
        ..Default::default()
    });
    let mut style = crate::Style::default();
    style.flags.set_bold(true);
    let data = CellCopy {
        cell: Cell::init('A'),
        suffix: None,
        style: Some(super::StyleCopy::Foreign(style)),
        link: None,
        managed: true,
    };
    assert_eq!(
        page.copy_cell(data, CellPosition { x: 0, y: 0 }),
        Err(crate::page::StorageError::StyleSetOutOfMemory)
    );
    assert!(page.get_row(0).styled());
    assert_eq!(page.get_cells(0)[0].style_id(), 0);
}
