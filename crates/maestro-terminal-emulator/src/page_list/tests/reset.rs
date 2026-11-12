//! Reset releases historical pages while preserving retained handle ownership.

use super::resize::{integrity, screen, text, write};
use super::*;

#[test]
fn history_reset() {
    let mut list = screen(80, 24);
    write(&mut list, 0, "A");
    list.reset();
    assert_eq!(list.viewport, Viewport::Active);
    assert_eq!(list.total_rows(), 24);
    assert_eq!(
        list.get_top_left(crate::point::Tag::Active),
        Pin {
            node: list.first,
            x: 0,
            y: 0,
            garbage: false
        }
    );
    assert!(text(&list, 0).chars().all(|cp| cp == '\0'));
    integrity(&list);
}

#[test]
fn history_reset_invalidates_stale_untracked_refs_even_if_node_memory_is_reused() {
    let mut list = screen(80, 24);
    let mut stale = Vec::new();
    for _ in 0..16 {
        stale.push(list.get_top_left(crate::point::Tag::Active));
        list.reset();
        assert!(list.node_is_valid(list.first));
        for pin in &stale {
            assert!(!list.node_is_valid(pin.node));
            assert!(!list.pin_is_valid(*pin));
            assert!(pin.row_and_cell(&list).is_none());
        }
    }
    integrity(&list);
}

#[test]
fn history_reset_across_two_pages() {
    let mut cols = 50;
    while PageList::initial_capacity(cols).rows >= 100 {
        cols += 50;
    }
    let mut list = screen(cols, 100);
    assert!(list.total_pages() > 1);
    let old = list.last;
    list.reset();
    assert_eq!(list.viewport, Viewport::Active);
    assert_eq!(list.total_rows(), 100);
    assert!(list.total_pages() > 1);
    assert!(!list.node_is_valid(old));
    integrity(&list);
}

#[test]
fn history_reset_moves_tracked_pins_and_marks_them_as_garbage() {
    let mut list = screen(80, 24);
    let pin = list
        .track_pin(
            list.pin(Point::Active(Coordinate { x: 42, y: 12 }))
                .unwrap(),
        )
        .unwrap();
    list.reset();
    let retained = *list.tracked_pin(pin).unwrap();
    assert!(retained.garbage);
    assert_eq!((retained.node, retained.x, retained.y), (list.first, 0, 0));
    assert!(!list.tracked_pin(list.viewport_pin).unwrap().garbage);
    assert_eq!(retained.row_and_cell(&list).unwrap().1.codepoint(), '\0');
    integrity(&list);
}

#[test]
fn history_clears_history() {
    let mut list = screen(80, 24);
    for _ in 0..30 {
        list.grow();
    }
    assert_eq!(list.total_rows(), 54);
    list.scroll(Scroll::Top);
    list.reset();
    assert_eq!(list.viewport, Viewport::Active);
    assert_eq!(list.total_rows(), 24);
    assert_eq!(list.history_rows(), 0);
    assert_eq!(
        list.get_top_left(crate::point::Tag::Active),
        Pin {
            node: list.first,
            x: 0,
            y: 0,
            garbage: false
        }
    );
    integrity(&list);
}
