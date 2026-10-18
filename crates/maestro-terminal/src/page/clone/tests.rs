//! Row summaries stay independent of managed cell ownership.

use super::{Cell, CellCopy, CellPosition, Page};

#[test]
fn row_metadata_unions_placeholder_summaries_only_for_partial_copies() {
    let mut page = Page::init(crate::page::layout::Capacity {
        cols: 2,
        rows: 2,
        ..Default::default()
    });
    for incoming in [false, true] {
        for surviving in [false, true] {
            let mut source = super::Row::default();
            source.set_kitty_virtual_placeholder(incoming);
            assert!(!source.managed_memory());
            for count in [1, 2] {
                page.get_row_mut(1).set_kitty_virtual_placeholder(surviving);
                page.copy_row_metadata(source, 1, count);
                assert_eq!(
                    page.get_row(1).kitty_virtual_placeholder(),
                    incoming || (count == 1 && surviving),
                    "incoming={incoming}, surviving={surviving}, count={count}"
                );
            }
        }
    }
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
    };
    assert_eq!(
        page.copy_cell(data, CellPosition { x: 0, y: 0 }),
        Err(crate::page::StorageError::StyleSetOutOfMemory)
    );
    assert!(page.get_row(0).styled());
    assert_eq!(page.get_cells(0)[0].style_id(), 0);
}
