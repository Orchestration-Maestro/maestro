//! Retained prompt, style, cluster and placeholder metadata during reflow.

use super::reflow::{flags, reflow};
use super::resize::{integrity, nz, screen, text, write};
use super::*;
use crate::page::CellPosition;

#[test]
fn history_resize_reflow_less_cols_copy_style() {
    let mut list = PageList::init(nz(4), nz(2), Some(0));
    assert_eq!(list.total_pages(), 1);
    write(&mut list, 0, "\0\u{1}\u{2}");
    let mut style = crate::Style::default();
    style.flags.set_bold(true);
    let page = list.page_mut(list.first).unwrap();
    for x in 0..3 {
        let id = page.styles.add(style).unwrap();
        page.get_cells_mut(0)[x].set_style_id(id);
    }
    page.get_row_mut(0).set_styled(true);
    reflow(&mut list, 2);
    assert_eq!((list.cols(), list.total_rows()), (2, 2));
    for (y, expected) in ["\0\u{1}", "\u{2}\0"].into_iter().enumerate() {
        assert_eq!(text(&list, y), expected);
        let pin = list.pin_at_row(y, 0).unwrap();
        assert_ne!(pin.row_and_cell(&list).unwrap().1.style_id(), 0);
        assert!(pin.row_and_cell(&list).unwrap().0.styled());
        assert!(
            pin.style(&list, *pin.row_and_cell(&list).unwrap().1)
                .unwrap()
                .flags
                .bold()
        );
    }
    integrity(&list);
}

/// A screen with placeholders only in the specified first-row columns.
fn placeholders(columns: std::ops::Range<usize>) -> PageList {
    let mut list = PageList::init(nz(4), nz(2), Some(0));
    assert_eq!(list.total_pages(), 1);
    let page = list.page_mut(list.first).unwrap();
    for x in columns {
        page.get_cells_mut(0)[x] = crate::page::cell::Cell::init('\u{10eeee}');
    }
    page.get_row_mut(0).set_kitty_virtual_placeholder(true);
    list
}

/// Read the placeholder-presence flag from a retained row.
fn placeholder(list: &PageList, y: usize) -> bool {
    list.pin_at_row(y, 0)
        .unwrap()
        .row_and_cell(list)
        .unwrap()
        .0
        .kitty_virtual_placeholder()
}

#[test]
fn history_resize_reflow_less_cols_copy_kitty_placeholder() {
    let mut list = placeholders(0..3);
    reflow(&mut list, 2);
    assert_eq!((list.cols(), list.total_rows()), (2, 2));
    assert!(placeholder(&list, 0));
    assert!(placeholder(&list, 1));
    assert_eq!(text(&list, 0), "\u{10eeee}\u{10eeee}");
    assert_eq!(text(&list, 1), "\u{10eeee}\0");
    integrity(&list);
}

#[test]
fn history_resize_reflow_more_cols_clears_kitty_placeholder() {
    let mut list = placeholders(0..3);
    reflow(&mut list, 2);
    reflow(&mut list, 4);
    assert_eq!((list.cols(), list.total_rows()), (4, 2));
    assert!(placeholder(&list, 0));
    assert!(!placeholder(&list, 1));
    assert_eq!(text(&list, 0), "\u{10eeee}\u{10eeee}\u{10eeee}\0");
    assert_eq!(text(&list, 1), "\0\0\0\0");
    integrity(&list);
}

#[test]
fn history_resize_reflow_wrap_moves_kitty_placeholder() {
    let mut list = placeholders(2..3);
    reflow(&mut list, 2);
    assert_eq!((list.cols(), list.total_rows()), (2, 2));
    assert!(!placeholder(&list, 0));
    assert!(placeholder(&list, 1));
    assert_eq!(text(&list, 0), "\0\0");
    assert_eq!(text(&list, 1), "\u{10eeee}\0");
    integrity(&list);
}

#[test]
fn history_resize_reflow_grapheme_map_capacity_exceeded() {
    let mut list = screen(4, 10);
    assert_eq!(list.total_pages(), 1);
    let cap = list.page(list.first).unwrap().grapheme_capacity();
    let count = cap / 2 + cap / 4;
    let rows = usize::from(list.page(list.first).unwrap().capacity.rows);
    while list.total_rows() < rows + count {
        if list.total_rows() == rows {
            assert_eq!(list.total_pages(), 1);
        }
        list.grow();
    }
    assert_eq!(list.total_pages(), 2);
    assert_ne!(list.first, list.last);
    assert_eq!(list.nodes.get(list.first).unwrap().next, Some(list.last));
    let mut pins = Vec::new();
    for (start, cp, suffix) in [(rows - count, 'A', '\u{301}'), (rows, 'B', '\u{302}')] {
        for y in start..start + count {
            write(&mut list, y, &cp.to_string());
            let pin = list.pin_at_row(y, 0).unwrap();
            list.page_mut(pin.node)
                .unwrap()
                .append_grapheme(CellPosition { x: 0, y: pin.y }, suffix)
                .unwrap();
            pins.push((list.track_pin(pin).unwrap(), cp, suffix));
        }
    }
    reflow(&mut list, 2);
    assert_eq!(list.cols(), 2);
    for (handle, cp, suffix) in pins {
        let pin = *list.tracked_pin(handle).unwrap();
        assert_eq!(pin.row_and_cell(&list).unwrap().1.codepoint(), cp);
        assert_eq!(
            pin.grapheme(&list, CellPosition { x: pin.x, y: pin.y }),
            Some(&[suffix][..])
        );
    }
    integrity(&list);
}

#[test]
fn history_resize_grow_cols_with_unwrap_fixes_viewport_pin() {
    let mut list = screen(2, 10);
    assert_eq!(list.total_pages(), 1);
    for _ in 0..30 {
        list.grow();
    }
    assert_eq!(list.total_rows(), 40);
    for y in 0..40 {
        write(&mut list, y, "AA");
        let pin = list.pin_at_row(y, 0).unwrap();
        let row = list.page_mut(pin.node).unwrap().get_row_mut(pin.y);
        row.set_wrap(y % 2 == 0);
        row.set_wrap_continuation(y % 2 == 1);
    }
    list.scroll(Scroll::Row(28));
    assert_eq!(list.viewport, Viewport::Pin);
    assert!(list.get_bottom_right(crate::point::Tag::Viewport).is_some());
    reflow(&mut list, 4);
    assert_eq!((list.cols(), list.total_rows()), (4, 20));
    assert_eq!(list.viewport, Viewport::Active);
    assert!(list.get_bottom_right(crate::point::Tag::Viewport).is_some());
    assert_eq!(flags(&list, 19), (false, false));
    assert_eq!(text(&list, 19), "AAAA");
    integrity(&list);
}
