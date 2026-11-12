//! Dimension changes, retained cursors and managed ownership.

use super::*;
use crate::page::{
    CellPosition,
    cell::{Cell as PackedCell, Content, Wide},
};
use crate::point::Tag;

/// Initialize a small screen with unrestricted retained history.
pub(super) fn screen(cols: u16, rows: u16) -> PageList {
    PageList::init(nz(cols), nz(rows), None)
}

/// Convert a positive fixture dimension.
pub(super) fn nz(value: u16) -> NonZeroU16 {
    NonZeroU16::new(value).unwrap()
}

/// Read a retained row's text, including its NUL padding.
pub(super) fn text(list: &PageList, row: usize) -> String {
    let pin = list.pin_at_row(row, 0).unwrap();
    list.page(pin.node)
        .unwrap()
        .get_cells(pin.y)
        .iter()
        .map(|c| c.codepoint())
        .collect()
}

/// Fill one row, preserving managed storage for callers that add it afterwards.
pub(super) fn write(list: &mut PageList, row: usize, value: &str) {
    let pin = list.pin_at_row(row, 0).unwrap();
    for (cell, cp) in list
        .page_mut(pin.node)
        .unwrap()
        .get_cells_mut(pin.y)
        .iter_mut()
        .zip(value.chars())
    {
        *cell = PackedCell::init(cp);
    }
}

/// Resolve a tracked location into the active region.
pub(super) fn active(list: &PageList, pin: TrackedPin) -> Option<Coordinate> {
    list.point_from_pin(Tag::Active, *list.tracked_pin(pin).unwrap())
        .map(Point::coord)
}

/// Apply a non-reflow height change.
fn height(list: &mut PageList, rows: u16) {
    list.resize(Resize {
        rows: Some(nz(rows)),
        reflow: false,
        ..Resize::default()
    })
    .unwrap();
}

/// Apply a non-reflow width change.
pub(super) fn width(list: &mut PageList, cols: u16) {
    list.resize(Resize {
        cols: Some(nz(cols)),
        reflow: false,
        ..Resize::default()
    })
    .unwrap();
}

/// Verify row widths, cached row count and retained-handle bounds.
pub(super) fn integrity(list: &PageList) {
    let mut next = Some(list.first);
    let mut previous = None;
    let mut count = 0;
    let mut charge = 0;
    while let Some(key) = next {
        let node = list.nodes.get(key).unwrap();
        assert_eq!(node.prev, previous);
        assert_eq!(node.page.size.cols, list.cols());
        assert!(node.page.size.rows > 0 && node.page.size.rows <= node.page.capacity.rows);
        for y in 0..node.page.size.rows {
            assert_eq!(node.page.get_cells(y).len(), usize::from(list.cols()));
        }
        count += usize::from(node.page.size.rows);
        charge += PageList::page_charge(node.page.capacity);
        previous = Some(key);
        next = node.next;
    }
    assert_eq!(previous, Some(list.last));
    assert_eq!(count, list.total_rows());
    assert_eq!(charge, list.charged_bytes());
    assert!(count >= usize::from(list.rows()));
    assert!(list.tracked_pins().all(|(_, pin)| list.pin_is_valid(*pin)));
}

/// Assert the borrowed origin projects to its full retained screen coordinate.
pub(super) fn screen_origin(list: &PageList, tag: Tag, y: u32) {
    let point = match tag {
        Tag::Active => Point::Active(Coordinate::default()),
        Tag::Viewport => Point::Viewport(Coordinate::default()),
        _ => unreachable!(),
    };
    let cell = list.get_cell(point).unwrap();
    assert_eq!(
        list.point_from_pin(
            Tag::Screen,
            Pin {
                node: cell.node,
                x: cell.col_idx,
                y: cell.row_idx,
                garbage: false,
            },
        ),
        Some(Point::Screen(Coordinate { x: 0, y }))
    );
}

#[test]
fn history_resize_no_reflow_more_rows() {
    let mut list = PageList::init(nz(10), nz(3), Some(0));
    assert_eq!(list.total_rows(), 3);
    let pin = list.track_pin(list.pin_at_row(2, 0).unwrap()).unwrap();
    write(&mut list, 0, "A");
    height(&mut list, 10);
    assert_eq!((list.rows(), list.total_rows()), (10, 10));
    assert_eq!(active(&list, pin), Some(Coordinate { x: 0, y: 2 }));
    assert!(text(&list, 0).starts_with('A'));
    screen_origin(&list, Tag::Active, 0);
    integrity(&list);
}

#[test]
fn history_resize_no_reflow_more_rows_with_history() {
    let mut list = screen(10, 3);
    for _ in 0..50 {
        list.grow();
    }
    screen_origin(&list, Tag::Active, 50);
    write(&mut list, 50, "A");
    let pin = list.track_pin(list.pin_at_row(52, 0).unwrap()).unwrap();
    height(&mut list, 5);
    assert_eq!(
        (list.rows(), list.total_rows(), list.history_rows()),
        (5, 53, 48)
    );
    assert_eq!(active(&list, pin), Some(Coordinate { x: 0, y: 4 }));
    assert!(text(&list, 50).starts_with('A'));
    screen_origin(&list, Tag::Active, 48);
    integrity(&list);
}

/// A nonblank row at every original location prevents trailing trimming.
fn full_rows() -> PageList {
    let mut list = PageList::init(nz(10), nz(10), Some(0));
    assert_eq!(list.total_rows(), 10);
    assert_eq!(list.first, list.last);
    for row in 0..10 {
        write(&mut list, row, "A");
    }
    list
}

#[test]
fn history_resize_no_reflow_less_rows() {
    let mut list = full_rows();
    height(&mut list, 5);
    assert_eq!(
        (list.rows(), list.total_rows(), list.history_rows()),
        (5, 10, 5)
    );
    assert!((0..10).all(|row| text(&list, row).starts_with('A')));
    screen_origin(&list, Tag::Active, 5);
    integrity(&list);
}

#[test]
fn history_resize_no_reflow_one_rows() {
    let mut list = full_rows();
    height(&mut list, 1);
    assert_eq!(
        (list.rows(), list.total_rows(), list.history_rows()),
        (1, 10, 9)
    );
    assert!(text(&list, 9).starts_with('A'));
    screen_origin(&list, Tag::Active, 9);
    integrity(&list);
}

/// Write distinct row markers so a retained cursor cannot resolve a neighbour.
fn numbered_rows() -> PageList {
    let mut list = full_rows();
    for y in 0..10 {
        write(
            &mut list,
            y,
            &char::from_u32(u32::try_from(y).unwrap())
                .unwrap()
                .to_string(),
        );
    }
    list
}

#[test]
fn history_resize_no_reflow_less_rows_cursor_on_bottom() {
    let mut list = numbered_rows();
    let pin = list.track_pin(list.pin_at_row(9, 0).unwrap()).unwrap();
    assert_eq!(
        list.get_cell(Point::Active(active(&list, pin).unwrap()))
            .unwrap()
            .cell
            .codepoint(),
        '\t'
    );
    height(&mut list, 5);
    assert_eq!(active(&list, pin), Some(Coordinate { x: 0, y: 4 }));
    assert_eq!(list.total_rows(), 10);
    assert!(text(&list, 0).starts_with('\0'));
    assert_eq!(list.rows(), 5);
    screen_origin(&list, Tag::Active, 5);
    integrity(&list);
}

#[test]
fn history_resize_no_reflow_less_rows_cursor_in_scrollback() {
    let mut list = numbered_rows();
    let pin = list.track_pin(list.pin_at_row(2, 0).unwrap()).unwrap();
    assert_eq!(
        list.get_cell(Point::Active(active(&list, pin).unwrap()))
            .unwrap()
            .cell
            .codepoint(),
        '\u{2}'
    );
    height(&mut list, 5);
    assert_eq!(active(&list, pin), None);
    assert_eq!(
        list.point_from_pin(Tag::Screen, *list.tracked_pin(pin).unwrap()),
        Some(Point::Screen(Coordinate { x: 0, y: 2 }))
    );
    assert_eq!((list.rows(), list.total_rows()), (5, 10));
    assert!(text(&list, 9).starts_with('\t'));
    screen_origin(&list, Tag::Active, 5);
    integrity(&list);
}

/// Text followed by background-only rows, which are text-free for trimming.
fn background_rows(rows: u16) -> PageList {
    let mut list = PageList::init(nz(10), nz(rows), Some(0));
    assert_eq!(list.first, list.last);
    write(&mut list, 0, "A");
    let page = list.page_mut(list.first).unwrap();
    for y in 1..rows {
        page.get_cells_mut(y)[0].set_content(Content::BgColorRgb(crate::color::Rgb {
            r: 255,
            g: 0,
            b: 0,
        }));
        page.get_cells_mut(y)[1].set_content(Content::BgColorPalette(1));
    }
    list
}

#[test]
fn history_resize_no_reflow_less_rows_trims_blank_lines() {
    let mut list = background_rows(5);
    assert_eq!(
        list.get_cell(Point::Active(Coordinate::default()))
            .unwrap()
            .cell
            .codepoint(),
        'A'
    );
    let pin = list.track_pin(list.pin_at_row(0, 0).unwrap()).unwrap();
    height(&mut list, 2);
    assert_eq!((list.rows(), list.total_rows()), (2, 2));
    assert_eq!(active(&list, pin), Some(Coordinate::default()));
    screen_origin(&list, Tag::Active, 0);
    assert!(text(&list, 0).starts_with('A'));
    integrity(&list);
}

#[test]
fn history_resize_no_reflow_less_rows_trims_blank_lines_cursor_in_blank_line() {
    let mut list = background_rows(5);
    let pin = list.track_pin(list.pin_at_row(3, 0).unwrap()).unwrap();
    height(&mut list, 2);
    assert_eq!((list.rows(), list.total_rows()), (2, 4));
    assert_eq!(active(&list, pin), Some(Coordinate { x: 0, y: 1 }));
    assert!(text(&list, 0).starts_with('A'));
    integrity(&list);
}

#[test]
fn history_resize_no_reflow_less_rows_trims_blank_lines_erases_pages() {
    let mut list = screen(100, 5);
    let first = list.first;
    assert_eq!(list.first, list.last);
    let rows = list.page(first).unwrap().capacity.rows + 10;
    height(&mut list, rows);
    let removed = list.last;
    assert_eq!(list.total_pages(), 2);
    write(&mut list, 0, "A");
    height(&mut list, 5);
    assert_eq!(
        (list.rows(), list.total_rows(), list.total_pages()),
        (5, 5, 1)
    );
    assert!(list.node_is_valid(first));
    assert!(!list.node_is_valid(removed));
    assert!(text(&list, 0).starts_with('A'));
    integrity(&list);
}

#[test]
fn history_resize_no_reflow_more_rows_extends_blank_lines() {
    let mut list = background_rows(3);
    height(&mut list, 7);
    assert_eq!(
        (list.rows(), list.total_rows(), list.history_rows()),
        (7, 7, 0)
    );
    assert!(text(&list, 0).starts_with('A'));
    assert_eq!(
        list.page(list.first).unwrap().get_cells(1)[1].content(),
        Content::BgColorPalette(1)
    );
    screen_origin(&list, Tag::Active, 0);
    integrity(&list);
}

#[test]
fn history_resize_no_reflow_more_rows_contains_viewport() {
    let mut list = PageList::init(nz(5), nz(5), Some(1));
    assert_eq!(list.first, list.last);
    list.grow();
    assert_eq!((list.rows(), list.total_rows()), (5, 6));
    list.scroll(Scroll::DeltaRow(-1));
    assert_eq!(list.viewport, Viewport::Top);
    height(&mut list, 7);
    assert_eq!((list.rows(), list.total_rows()), (7, 7));
    assert_eq!(list.viewport, Viewport::Top);
    integrity(&list);
}

#[test]
fn history_resize_no_reflow_less_cols_pin_in_trimmed_cols() {
    let mut list = screen(10, 10);
    write(&mut list, 0, "abcdefghij");
    let pin = list.track_pin(list.pin_at_row(2, 8).unwrap()).unwrap();
    let node = list.first;
    width(&mut list, 5);
    assert_eq!(text(&list, 0), "abcde");
    assert_eq!(active(&list, pin), Some(Coordinate { x: 4, y: 2 }));
    assert_eq!((list.cols(), list.total_rows()), (5, 10));
    assert!(list.node_is_valid(node));
    integrity(&list);
}

#[test]
fn history_resize_no_reflow_less_cols_clears_graphemes() {
    let mut list = screen(10, 10);
    write(&mut list, 0, "abcdefghij");
    let page = list.page_mut(list.first).unwrap();
    page.set_graphemes(CellPosition { x: 8, y: 0 }, &['\u{301}'])
        .unwrap();
    assert_eq!(page.grapheme_count(), 1);
    width(&mut list, 5);
    assert_eq!((list.cols(), list.total_rows()), (5, 10));
    assert_eq!(list.page(list.first).unwrap().grapheme_count(), 0);
    width(&mut list, 10);
    assert_eq!(text(&list, 0), "abcde\0\0\0\0\0");
    assert!(!list.page(list.first).unwrap().get_row(0).grapheme());
    integrity(&list);
}

#[test]
fn history_resize_no_reflow_less_cols_then_more_cols() {
    let mut blank = PageList::init(nz(5), nz(3), Some(0));
    width(&mut blank, 2);
    assert_eq!(blank.cols(), 2);
    width(&mut blank, 5);
    assert_eq!((blank.cols(), blank.total_rows()), (5, 3));
    integrity(&blank);
    let mut list = screen(5, 3);
    write(&mut list, 0, "ABCDE");
    let node = list.first;
    width(&mut list, 2);
    assert_eq!(list.cols(), 2);
    width(&mut list, 5);
    assert_eq!(list.cols(), 5);
    assert_eq!(text(&list, 0), "AB\0\0\0");
    assert_eq!(list.first, node);
    assert_eq!(list.total_rows(), 3);
    assert_eq!(list.cols(), 5);
    integrity(&list);
}

#[test]
fn resize_failure_keeps_source_and_retained_locations() {
    let mut list = screen(3, 3);
    let node = list.first;
    let pin = list.track_pin(list.pin_at_row(1, 0).unwrap()).unwrap();
    let page = list.page_mut(node).unwrap();
    let id = page.styles.add(crate::Style::default()).unwrap();
    page.get_cells_mut(1)[0].set_style_id(id);
    page.get_row_mut(1).set_styled(true);
    page.capacity.styles = 0;
    assert_eq!(
        list.resize(Resize {
            cols: Some(nz(6)),
            reflow: false,
            ..Resize::default()
        }),
        Err(crate::page::StorageError::StyleSetOutOfMemory)
    );
    assert_eq!(list.cols(), 3);
    assert_eq!(list.first, node);
    assert_eq!(list.tracked_pin(pin).unwrap().node, node);
    assert_eq!(list.total_pages(), 1);
    integrity(&list);
}

#[test]
fn history_resize_no_reflow_more_cols_with_spacer_head() {
    spacer_growth(2, 3);
}

#[test]
fn history_resize_no_reflow_grow_cols_fast_path_with_spacer_head() {
    let mut list = screen(10, 3);
    width(&mut list, 5);
    assert_eq!(list.cols(), 5);
    write(&mut list, 0, "x");
    let page = list.page_mut(list.first).unwrap();
    for y in 0..2 {
        page.get_cells_mut(y)[4].set_wide(Wide::SpacerHead);
        page.get_row_mut(y).set_wrap(true);
    }
    width(&mut list, 10);
    assert_eq!(list.cols(), 10);
    let page = list.page(list.first).unwrap();
    for y in 0..2 {
        assert_eq!(page.get_cells(y)[4].wide(), Wide::Narrow);
        assert!(!page.get_row(y).wrap());
    }
    integrity(&list);
    spacer_growth(4, 3);
}

/// Exercise both a fresh allocation and retained-capacity widening with spacers.
fn spacer_growth(initial: u16, target: u16) {
    let mut list = screen(initial, 3);
    assert_eq!(list.first, list.last);
    width(&mut list, 2);
    write(&mut list, 0, "x");
    let page = list.page_mut(list.first).unwrap();
    page.get_row_mut(0).set_wrap(true);
    page.get_cells_mut(0)[1].set_wide(Wide::SpacerHead);
    page.get_cells_mut(1)[0] = PackedCell::init('😀');
    page.get_cells_mut(1)[0].set_wide(Wide::Wide);
    page.get_cells_mut(1)[1].set_wide(Wide::SpacerTail);
    let pin = list.track_pin(list.pin_at_row(1, 0).unwrap()).unwrap();
    width(&mut list, target);
    assert_eq!((list.cols(), list.total_rows()), (target, 3));
    assert_eq!(list.first, list.last);
    assert_eq!(text(&list, 0), "x\0\0");
    let page = list.page(list.first).unwrap();
    for x in 0..3 {
        assert_eq!(page.get_cells(0)[x].wide(), Wide::Narrow);
    }
    assert_eq!(page.get_cells(1)[0].wide(), Wide::Wide);
    assert_eq!(page.get_cells(1)[1].wide(), Wide::SpacerTail);
    assert_eq!(active(&list, pin), Some(Coordinate { x: 0, y: 1 }));
    integrity(&list);
}

#[test]
fn history_resize_no_reflow_more_cols_forces_smaller_cap() {
    let cap = PageList::initial_capacity(100);
    let cap2 = PageList::initial_capacity(500);
    assert_eq!(cap2.cols, 500);
    assert!(cap2.rows < cap.rows);
    let mut list = screen(100, cap.rows);
    assert_eq!(list.first, list.last);
    for y in 0..usize::from(cap.rows) {
        write(&mut list, y, &"A".repeat(100));
    }
    let pin = list
        .track_pin(list.pin_at_row(usize::from(cap.rows - 1), 99).unwrap())
        .unwrap();
    let old = list.first;
    width(&mut list, 500);
    assert_eq!(list.total_rows(), usize::from(cap.rows));
    assert!(list.total_pages() > 1);
    assert!(!list.node_is_valid(old));
    assert!((0..list.total_rows()).all(|y| text(&list, y).starts_with(&"A".repeat(100))));
    assert_eq!(
        active(&list, pin),
        Some(Coordinate {
            x: 99,
            y: u32::from(cap.rows - 1)
        })
    );
    assert_eq!(list.cols(), 500);
    integrity(&list);
}

/// Check an empty screen's dimensions and every retained row width after resizing.
fn blank_resize(initial: (u16, u16), target: (u16, u16), total: usize) {
    let mut list = PageList::init(nz(initial.0), nz(initial.1), Some(0));
    list.resize(Resize {
        cols: (initial.0 != target.0).then(|| nz(target.0)),
        rows: (initial.1 != target.1).then(|| nz(target.1)),
        reflow: false,
        ..Resize::default()
    })
    .unwrap();
    assert_eq!(
        (list.cols(), list.rows(), list.total_rows()),
        (target.0, target.1, total)
    );
    integrity(&list);
}

#[test]
fn history_resize_no_reflow_less_cols() {
    blank_resize((10, 10), (5, 10), 10);
    let mut list = screen(10, 10);
    write(&mut list, 0, "ABCDEFGHIJ");
    write(&mut list, 9, "KLMNOPQRST");
    width(&mut list, 5);
    assert_eq!(list.total_rows(), 10);
    assert_eq!(text(&list, 0), "ABCDE");
    assert_eq!(text(&list, 9), "KLMNO");
    assert_eq!(list.cols(), 5);
    integrity(&list);
}

#[test]
fn history_resize_no_reflow_more_cols() {
    blank_resize((5, 3), (10, 3), 3);
    let mut list = screen(5, 3);
    write(&mut list, 0, "ABCDE");
    write(&mut list, 2, "VWXYZ");
    let pin = list.track_pin(list.pin_at_row(2, 4).unwrap()).unwrap();
    width(&mut list, 10);
    assert_eq!(list.total_rows(), 3);
    assert_eq!(text(&list, 0), "ABCDE\0\0\0\0\0");
    assert_eq!(text(&list, 2), "VWXYZ\0\0\0\0\0");
    assert_eq!(active(&list, pin), Some(Coordinate { x: 4, y: 2 }));
    assert_eq!(list.cols(), 10);
    integrity(&list);
}

#[test]
fn history_resize_no_reflow_more_cols_forces_less_rows_per_page() {
    assert!(PageList::initial_capacity(5).rows >= 150);
    let mut list = screen(5, 150);
    write(&mut list, 0, "first");
    write(&mut list, 149, "last");
    let pin = list.track_pin(list.pin_at_row(149, 0).unwrap()).unwrap();
    let mut cols = 50;
    while PageList::initial_capacity(cols).rows >= 150 {
        cols += 50;
    }
    width(&mut list, cols);
    assert_eq!((list.cols(), list.total_rows()), (cols, 150));
    assert_full_prefix(&list);
    let rows = list.page(list.first).unwrap().size.rows;
    cols += 50;
    while PageList::initial_capacity(cols).rows >= rows {
        cols += 50;
    }
    width(&mut list, cols);
    assert_eq!((list.cols(), list.total_rows()), (cols, 150));
    assert_full_prefix(&list);
    assert_eq!(list.total_rows(), 150);
    assert!(text(&list, 0).starts_with("first"));
    assert!(text(&list, 149).starts_with("last"));
    assert_eq!(active(&list, pin), Some(Coordinate { x: 0, y: 149 }));
    integrity(&list);
}

/// Resizing backfills intermediate pages rather than stranding row capacity.
fn assert_full_prefix(list: &PageList) {
    let mut next = Some(list.first);
    while let Some(key) = next {
        let node = list.nodes.get(key).unwrap();
        if key != list.last {
            assert_eq!(node.page.size.rows, node.page.capacity.rows);
        }
        next = node.next;
    }
}

#[test]
fn history_resize_no_reflow_less_rows_and_cols() {
    blank_resize((10, 10), (5, 7), 7);
    let mut list = screen(10, 10);
    write(&mut list, 0, "ABCDEFGHIJ");
    let pin = list.track_pin(list.pin_at_row(0, 4).unwrap()).unwrap();
    list.resize(Resize {
        cols: Some(nz(5)),
        rows: Some(nz(7)),
        reflow: false,
        ..Resize::default()
    })
    .unwrap();
    assert_eq!((list.cols(), list.rows(), list.total_rows()), (5, 7, 7));
    assert_eq!(text(&list, 0), "ABCDE");
    assert_eq!(active(&list, pin), Some(Coordinate { x: 4, y: 0 }));
    integrity(&list);
}

#[test]
fn history_resize_no_reflow_more_rows_and_less_cols() {
    blank_resize((10, 10), (5, 20), 20);
    let mut list = screen(10, 10);
    write(&mut list, 0, "ABCDEFGHIJ");
    let pin = list.track_pin(list.pin_at_row(0, 4).unwrap()).unwrap();
    list.resize(Resize {
        cols: Some(nz(5)),
        rows: Some(nz(20)),
        reflow: false,
        ..Resize::default()
    })
    .unwrap();
    assert_eq!((list.cols(), list.rows(), list.total_rows()), (5, 20, 20));
    assert_eq!(text(&list, 0), "ABCDE");
    assert_eq!(active(&list, pin), Some(Coordinate { x: 4, y: 0 }));
    integrity(&list);
}

#[test]
fn history_resize_no_reflow_empty_screen() {
    let mut list = screen(5, 5);
    let pin = list.track_pin(list.pin_at_row(4, 4).unwrap()).unwrap();
    list.resize(Resize {
        cols: Some(nz(10)),
        rows: Some(nz(10)),
        reflow: false,
        ..Resize::default()
    })
    .unwrap();
    assert_eq!((list.cols(), list.rows(), list.total_rows()), (10, 10, 10));
    assert!((0..10).all(|row| text(&list, row).chars().all(|cp| cp == '\0')));
    assert_eq!(active(&list, pin), Some(Coordinate { x: 4, y: 4 }));
    integrity(&list);
}

#[test]
fn history_resize_no_reflow_more_rows_adds_blank_rows_if_cursor_at_bottom() {
    let mut list = screen(5, 3);
    list.grow();
    list.grow();
    assert_eq!(list.first, list.last);
    screen_origin(&list, Tag::Active, 2);
    for row in 0..5 {
        write(
            &mut list,
            row,
            &char::from_u32(u32::try_from(row).unwrap())
                .unwrap()
                .to_string(),
        );
    }
    let pin = list.track_pin(list.pin_at_row(3, 0).unwrap()).unwrap();
    let cursor = active(&list, pin).unwrap();
    assert_eq!(
        list.get_cell(Point::Active(cursor))
            .unwrap()
            .cell
            .codepoint(),
        '\u{3}'
    );
    list.resize(Resize {
        rows: Some(nz(10)),
        reflow: false,
        cursor: Some(Cursor {
            x: 0,
            y: 1,
            pin: None,
        }),
        ..Resize::default()
    })
    .unwrap();
    assert_eq!(
        (
            list.cols(),
            list.rows(),
            list.total_rows(),
            list.history_rows()
        ),
        (5, 10, 12, 2)
    );
    assert_eq!(active(&list, pin), Some(cursor));
    screen_origin(&list, Tag::Active, 2);
    for row in 2..5 {
        assert_eq!(
            text(&list, row).chars().next(),
            char::from_u32(u32::try_from(row).unwrap())
        );
    }
    integrity(&list);
}

#[test]
fn history_resize_no_reflow_more_cols_remaps_pins_in_backfill_path() {
    let capacity = PageList::initial_capacity(5);
    let mut list = screen(5, capacity.rows);
    list.grow();
    let original = list.last;
    assert_ne!(list.first, original);
    list.erase_history(Some(Point::History(Coordinate::default())));
    assert!(
        list.page(list.first).unwrap().size.rows < list.page(list.first).unwrap().capacity.rows
    );
    assert!(list.page(original).unwrap().capacity.cols < PageList::initial_capacity(6).cols);
    let pin = list
        .track_pin(Pin {
            node: original,
            x: 0,
            y: 0,
            garbage: false,
        })
        .unwrap();
    list.page_mut(original).unwrap().get_cells_mut(0)[0] = PackedCell::init('X');
    width(&mut list, 6);
    let retained = *list.tracked_pin(pin).unwrap();
    assert_ne!(retained.node, original);
    assert!(list.node_is_valid(retained.node));
    assert!(retained.y < list.page(retained.node).unwrap().size.rows);
    assert_eq!(
        retained.row_and_cell(&list).unwrap().1.content(),
        Content::Codepoint('X')
    );
    assert!(!list.node_is_valid(original));
    integrity(&list);
}
