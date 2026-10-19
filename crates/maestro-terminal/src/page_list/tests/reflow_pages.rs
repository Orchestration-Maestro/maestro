//! Rewrapping through real source page boundaries and destination capacities.

use super::reflow::{flags, reflow};
use super::resize::{active, integrity, nz, screen, text, write};
use super::*;

/// Fill the first page and append the requested rows in its successor.
fn crossing(cols: u16, extra: usize) -> PageList {
    let mut list = screen(cols, 10);
    assert_eq!(list.total_pages(), 1);
    let cap = list.page(list.first).unwrap().capacity.rows;
    for _ in list.total_rows()..usize::from(cap) + extra {
        if list.total_rows() == usize::from(cap) {
            assert_eq!(list.total_pages(), 1);
        }
        list.grow();
    }
    assert_eq!(list.total_pages(), 2);
    let boundary = usize::from(cap) - 1;
    let line: String = (0..cols)
        .map(|x| char::from_u32(u32::from(x)).unwrap())
        .collect();
    for y in [boundary, boundary + 1] {
        write(&mut list, y, &line);
        let pin = list.pin_at_row(y, 0).unwrap();
        let row = list.page_mut(pin.node).unwrap().get_row_mut(pin.y);
        row.set_wrap(y == boundary);
        row.set_wrap_continuation(y != boundary);
    }
    list
}

#[test]
fn history_resize_reflow_more_cols_wrap_across_page_boundary() {
    let mut list = crossing(2, 1);
    let expected = list.total_rows() - 1;
    reflow(&mut list, 4);
    assert_eq!((list.cols(), list.total_rows()), (4, expected));
    assert_eq!(text(&list, expected - 2), "\0\0\0\0");
    assert_eq!(flags(&list, expected - 2), (false, false));
    assert_eq!(text(&list, expected - 1), "\0\u{1}\0\u{1}");
    assert_eq!(flags(&list, expected - 1), (false, false));
    integrity(&list);
}

#[test]
fn history_resize_reflow_more_cols_wrap_across_page_boundary_cursor_in_second_page() {
    let mut list = crossing(2, 1);
    let expected = list.total_rows() - 1;
    let pin = list
        .track_pin(list.pin_at_row(expected, 1).unwrap())
        .unwrap();
    assert_eq!(list.tracked_pin(pin).unwrap().node, list.last);
    reflow(&mut list, 4);
    assert_eq!((list.cols(), list.total_rows()), (4, expected));
    assert_eq!(active(&list, pin), Some(Coordinate { x: 3, y: 9 }));
    assert_eq!(text(&list, expected - 1), "\0\u{1}\0\u{1}");
    assert_eq!(flags(&list, expected - 1), (false, false));
    integrity(&list);
}

#[test]
fn history_resize_reflow_less_cols_wrap_across_page_boundary_cursor_in_second_page() {
    let mut list = crossing(5, 5);
    let source = list.total_rows();
    let pin = list
        .track_pin(list.pin_at_row(source - 5, 2).unwrap())
        .unwrap();
    assert_eq!(
        (
            list.tracked_pin(pin).unwrap().node,
            list.tracked_pin(pin).unwrap().y
        ),
        (list.last, 0)
    );
    list.resize(Resize {
        cols: Some(nz(4)),
        cursor: Some(Cursor {
            x: 2,
            y: 5,
            pin: None,
        }),
        ..Resize::default()
    })
    .unwrap();
    assert_eq!(list.cols(), 4);
    assert_eq!(active(&list, pin), Some(Coordinate { x: 3, y: 5 }));
    let top = list.history_rows();
    for (y, expected, bits) in [
        (3, "\0\0\0\0", (false, false)),
        (4, "\0\u{1}\u{2}\u{3}", (true, false)),
        (5, "\u{4}\0\u{1}\u{2}", (true, true)),
        (6, "\u{3}\u{4}\0\0", (false, true)),
        (7, "\0\0\0\0", (false, false)),
    ] {
        assert_eq!(text(&list, top + y), expected);
        assert_eq!(flags(&list, top + y), bits);
    }
    integrity(&list);
}

#[test]
fn history_resize_reflow_more_cols_creates_multiple_pages() {
    let mut cols = 100;
    while PageList::initial_capacity(cols).rows >= 100 {
        cols += 100;
    }
    let cap = PageList::initial_capacity(cols);
    let mut list = screen(cols, cap.rows);
    assert_eq!(list.total_pages(), 1);
    for y in 0..usize::from(cap.rows) {
        write(&mut list, y, "A");
        let row = list
            .page_mut(list.first)
            .unwrap()
            .get_row_mut(u16::try_from(y).unwrap());
        row.set_wrap(y % 2 == 0);
        row.set_wrap_continuation(y % 2 == 1);
    }
    let nextcap = PageList::initial_capacity(cols + 100);
    assert!(nextcap.rows < cap.rows);
    reflow(&mut list, cols + 100);
    assert_eq!(
        (list.cols(), list.total_rows()),
        (cols + 100, usize::from(cap.rows))
    );
    assert!(list.total_pages() > 1);
    let mut it = list.page_iterator(
        Direction::RightDown,
        Point::Screen(Coordinate::default()),
        None,
    );
    while let Some(chunk) = it.next(&list) {
        let page = list.page(chunk.node).unwrap();
        assert_eq!(
            (page.capacity.cols, page.capacity.rows),
            (nextcap.cols, nextcap.rows)
        );
    }
    assert!(text(&list, 0).starts_with('A'));
    integrity(&list);
}
