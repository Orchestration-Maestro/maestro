//! Managed-capacity replacement preserves content, links and retained state.

use super::resize::{integrity, nz, screen, text, width, write};
use super::*;

/// Capacity field selected by a managed-storage operation.
fn dimension(capacity: crate::page::layout::Capacity, field: IncreaseCapacity) -> u32 {
    match field {
        IncreaseCapacity::Styles => u32::from(capacity.styles),
        IncreaseCapacity::GraphemeBytes => capacity.grapheme_bytes,
        IncreaseCapacity::HyperlinkBytes => u32::from(capacity.hyperlink_bytes),
        IncreaseCapacity::StringBytes => capacity.string_bytes,
    }
}

/// Double the selected field without changing the two written rows.
pub(super) fn doubles(field: IncreaseCapacity) {
    let mut list = PageList::init(nz(2), nz(2), Some(0));
    for y in 0..2 {
        write(&mut list, y, "\0\u{1}");
    }
    let old = list.first;
    assert_eq!(list.first, list.last);
    let capacity = dimension(list.page(old).unwrap().capacity, field);
    let new = list.increase_capacity(old, Some(field)).unwrap();
    assert_eq!((list.first, list.last), (new, new));
    assert!(!list.node_is_valid(old));
    assert_eq!(
        dimension(list.page(new).unwrap().capacity, field),
        capacity * 2
    );
    assert_eq!(text(&list, 0), "\0\u{1}");
    assert_eq!(text(&list, 1), "\0\u{1}");
    integrity(&list);
}

#[test]
fn history_increase_capacity_to_increase_graphemes() {
    doubles(IncreaseCapacity::GraphemeBytes);
}

#[test]
fn history_increase_capacity_to_increase_hyperlinks() {
    doubles(IncreaseCapacity::HyperlinkBytes);
}

#[test]
fn history_increase_capacity_to_increase_string_bytes() {
    doubles(IncreaseCapacity::StringBytes);
}

#[test]
fn history_increase_capacity_tracked_pins() {
    let mut list = screen(2, 2);
    write(&mut list, 1, "AB");
    let old = list.first;
    let pin = list.track_pin(list.pin_at_row(1, 1).unwrap()).unwrap();
    assert_eq!(list.tracked_pin(pin).unwrap().node, old);
    let new = list
        .increase_capacity(old, Some(IncreaseCapacity::Styles))
        .unwrap();
    let retained = *list.tracked_pin(pin).unwrap();
    assert_eq!((retained.node, retained.x, retained.y), (new, 1, 1));
    assert_eq!(retained.row_and_cell(&list).unwrap().1.codepoint(), 'B');
    assert!(!list.node_is_valid(old));
    integrity(&list);
}

#[test]
fn history_increase_capacity_returns_out_of_space_at_max_capacity() {
    let mut list = screen(2, 2);
    write(&mut list, 0, "AB");
    loop {
        match list.increase_capacity(list.first, Some(IncreaseCapacity::Styles)) {
            Ok(_) => {}
            Err(error) => {
                assert_eq!(error, IncreaseCapacityError::OutOfSpace);
                assert_eq!(list.page(list.first).unwrap().capacity.styles, u16::MAX);
                break;
            }
        }
    }
    assert_eq!(text(&list, 0), "AB");
    integrity(&list);
}

#[test]
fn history_increase_capacity_after_col_shrink() {
    let mut list = screen(10, 2);
    write(&mut list, 0, "ABCDE");
    width(&mut list, 5);
    assert_eq!(list.cols(), 5);
    assert_eq!(list.page(list.first).unwrap().size.cols, 5);
    assert!(list.page(list.first).unwrap().capacity.cols >= 10);
    list.increase_capacity(list.first, Some(IncreaseCapacity::Styles))
        .unwrap();
    assert_eq!(list.page(list.first).unwrap().size.cols, 5);
    assert_eq!(list.cols(), 5);
    assert_eq!(text(&list, 0), "ABCDE");
    integrity(&list);
}

#[test]
fn history_increase_capacity_multi_page() {
    let mut list = screen(80, 24);
    let rows = usize::from(list.page(list.first).unwrap().capacity.rows);
    for _ in list.total_rows()..rows {
        assert!(list.grow().is_none());
    }
    assert!(list.grow().is_some());
    let tail = list.last;
    let first_cap = list.page(list.first).unwrap().capacity.styles;
    let last_cap = list.page(tail).unwrap().capacity.styles;
    write(&mut list, 0, "first");
    write(&mut list, rows, "last");
    list.increase_capacity(list.first, Some(IncreaseCapacity::Styles))
        .unwrap();
    assert_eq!(
        list.page(list.first).unwrap().capacity.styles,
        first_cap * 2
    );
    assert_eq!(list.page(tail).unwrap().capacity.styles, last_cap);
    assert!(text(&list, 0).starts_with("first"));
    assert!(text(&list, rows).starts_with("last"));
    assert_eq!(list.last, tail);
    integrity(&list);
}

#[test]
fn history_increase_capacity_preserves_dirty_flag() {
    let mut list = screen(2, 4);
    let page = list.page_mut(list.first).unwrap();
    page.dirty = true;
    for y in 0..4 {
        page.get_row_mut(y).set_dirty(y % 2 == 0);
    }
    let new = list
        .increase_capacity(list.first, Some(IncreaseCapacity::Styles))
        .unwrap();
    let page = list.page(new).unwrap();
    assert!(page.dirty);
    for y in 0..4 {
        assert_eq!(page.get_row(y).dirty(), y % 2 == 0);
    }
    integrity(&list);
}
