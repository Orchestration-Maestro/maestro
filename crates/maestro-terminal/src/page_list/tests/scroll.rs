//! Viewport movement retains content origins and coherent scrollbar offsets.

use super::resize::{screen, screen_origin, write};
use super::*;
use crate::point::Tag;

/// Create the reference geometry with the requested number of historical rows.
fn history(rows: usize) -> PageList {
    let mut list = screen(80, 24);
    append(&mut list, rows);
    list
}

/// Append through the real growth owner.
fn append(list: &mut PageList, rows: usize) {
    for _ in 0..rows {
        list.grow();
    }
}

/// Compare both the visible origin and the complete scrollbar observation.
fn viewport(list: &mut PageList, expected: Viewport, offset: usize) {
    assert_eq!(list.viewport, expected);
    screen_origin(list, Tag::Viewport, u32::try_from(offset).unwrap());
    let pin = list.pin(Point::Viewport(Coordinate::default())).unwrap();
    assert_eq!(
        list.point_from_pin(Tag::Screen, pin),
        Some(Point::Screen(Coordinate {
            x: 0,
            y: u32::try_from(offset).unwrap()
        }))
    );
    assert_eq!(
        list.scrollbar(),
        Scrollbar {
            total: list.total_rows(),
            offset,
            len: usize::from(list.rows())
        }
    );
}

#[test]
fn history_scrollbar_with_max_size_0_after_grow() {
    let mut list = PageList::init(
        NonZeroU16::new(80).unwrap(),
        NonZeroU16::new(24).unwrap(),
        Some(0),
    );
    append(&mut list, 10);
    assert_eq!(
        list.scrollbar(),
        Scrollbar {
            total: 24,
            offset: 0,
            len: 24
        }
    );
}

#[test]
fn history_scroll_with_max_size_0_no_history() {
    let mut list = PageList::init(
        NonZeroU16::new(80).unwrap(),
        NonZeroU16::new(24).unwrap(),
        Some(0),
    );
    append(&mut list, 10);
    let before = list.pin(Point::Viewport(Coordinate::default())).unwrap();
    screen_origin(&list, Tag::Viewport, 10);
    list.scroll(Scroll::DeltaRow(-5));
    assert_eq!(list.viewport, Viewport::Active);
    list.scroll(Scroll::Top);
    screen_origin(&list, Tag::Viewport, 10);
    assert_eq!(
        list.pin(Point::Viewport(Coordinate::default())),
        Some(before)
    );
}

#[test]
fn history_scroll_top() {
    let mut list = history(10);
    viewport(&mut list, Viewport::Active, 10);
    list.scroll(Scroll::Top);
    viewport(&mut list, Viewport::Top, 0);
    append(&mut list, 10);
    viewport(&mut list, Viewport::Top, 0);
    list.scroll(Scroll::Active);
    viewport(&mut list, Viewport::Active, 20);
}

#[test]
fn history_scroll_delta_row_back_overflow() {
    let mut list = history(10);
    viewport(&mut list, Viewport::Active, 10);
    list.scroll(Scroll::DeltaRow(-100));
    viewport(&mut list, Viewport::Top, 0);
    append(&mut list, 10);
    viewport(&mut list, Viewport::Top, 0);
}

#[test]
fn history_scroll_minimum_row_delta() {
    let mut list = screen(10, 3);
    append(&mut list, 1);
    list.scroll(Scroll::DeltaRow(isize::MIN));
    viewport(&mut list, Viewport::Top, 0);
}

#[test]
fn history_scroll_delta_row_forward() {
    let mut list = history(10);
    viewport(&mut list, Viewport::Active, 10);
    list.scroll(Scroll::Top);
    list.scroll(Scroll::DeltaRow(2));
    viewport(&mut list, Viewport::Pin, 2);
    append(&mut list, 10);
    viewport(&mut list, Viewport::Pin, 2);
}

#[test]
fn history_scroll_delta_row_forward_into_active() {
    let mut list = history(0);
    list.scroll(Scroll::DeltaRow(2));
    viewport(&mut list, Viewport::Active, 0);
}

#[test]
fn history_scroll_delta_row_back_without_space_preserves_active() {
    let mut list = history(0);
    list.scroll(Scroll::DeltaRow(-1));
    viewport(&mut list, Viewport::Active, 0);
}

#[test]
fn history_scroll_to_row_0() {
    let mut list = history(10);
    viewport(&mut list, Viewport::Active, 10);
    list.scroll(Scroll::Row(0));
    viewport(&mut list, Viewport::Top, 0);
    append(&mut list, 10);
    viewport(&mut list, Viewport::Top, 0);
}

#[test]
fn history_scroll_to_row_in_scrollback() {
    let mut list = history(20);
    viewport(&mut list, Viewport::Active, 20);
    list.scroll(Scroll::Row(5));
    viewport(&mut list, Viewport::Pin, 5);
    append(&mut list, 10);
    viewport(&mut list, Viewport::Pin, 5);
}

#[test]
fn history_scroll_to_row_in_middle() {
    let mut list = history(50);
    let midpoint = list.total_rows() / 2;
    list.scroll(Scroll::Row(midpoint));
    viewport(&mut list, Viewport::Pin, midpoint);
    append(&mut list, 10);
    viewport(&mut list, Viewport::Pin, midpoint);
}

#[test]
fn history_scroll_to_row_at_active_boundary() {
    let mut list = history(20);
    list.scroll(Scroll::Row(20));
    viewport(&mut list, Viewport::Active, 20);
    append(&mut list, 10);
    viewport(&mut list, Viewport::Active, 30);
}

#[test]
fn history_scroll_to_row_beyond_active() {
    let mut list = history(10);
    list.scroll(Scroll::Row(1000));
    viewport(&mut list, Viewport::Active, 10);
}

#[test]
fn history_scroll_to_row_without_scrollback() {
    let mut list = history(0);
    list.scroll(Scroll::Row(5));
    viewport(&mut list, Viewport::Active, 0);
}

#[test]
fn history_scroll_to_row_then_delta() {
    let mut list = history(30);
    list.scroll(Scroll::Row(10));
    viewport(&mut list, Viewport::Pin, 10);
    list.scroll(Scroll::DeltaRow(5));
    viewport(&mut list, Viewport::Pin, 15);
    list.scroll(Scroll::DeltaRow(-3));
    viewport(&mut list, Viewport::Pin, 12);
}

#[test]
fn history_scroll_to_row_with_cache_fast_path_down() {
    let mut list = history(50);
    list.scroll(Scroll::Row(10));
    viewport(&mut list, Viewport::Pin, 10);
    assert_eq!(list.viewport_pin_row_offset, Some(10));
    list.scroll(Scroll::Row(20));
    viewport(&mut list, Viewport::Pin, 20);
    append(&mut list, 10);
    viewport(&mut list, Viewport::Pin, 20);
}

#[test]
fn history_scroll_to_row_with_cache_fast_path_up() {
    let mut list = history(50);
    list.scroll(Scroll::Row(30));
    viewport(&mut list, Viewport::Pin, 30);
    assert_eq!(list.viewport_pin_row_offset, Some(30));
    list.scroll(Scroll::Row(15));
    viewport(&mut list, Viewport::Pin, 15);
    append(&mut list, 10);
    viewport(&mut list, Viewport::Pin, 15);
}

#[test]
fn history_scroll_clear() {
    let mut list = history(0);
    for y in 0..2 {
        write(&mut list, y, "A");
    }
    list.scroll_clear();
    viewport(&mut list, Viewport::Active, 2);
    for y in 0..2 {
        let pin = list.pin(Point::Screen(Coordinate { x: 0, y })).unwrap();
        assert_eq!(pin.row_and_cell(&list).unwrap().1.codepoint(), 'A');
    }
}

#[test]
fn scroll_clear_ignores_content_before_active_area() {
    let mut list = history(3);
    write(&mut list, 2, "H");
    let total = list.total_rows();
    list.scroll_clear();
    assert_eq!(list.total_rows(), total);
    viewport(&mut list, Viewport::Active, 3);
}

#[test]
fn scroll_clear_scans_across_page_boundary() {
    let mut list = screen(3, 3);
    write(&mut list, 1, "A");
    list.split(list.pin_at_row(2, 0).unwrap()).unwrap();
    assert_eq!(list.total_pages(), 2);
    list.scroll_clear();
    viewport(&mut list, Viewport::Active, 2);
    assert_eq!(
        list.get_cell(Point::Screen(Coordinate { x: 0, y: 1 }))
            .unwrap()
            .cell
            .codepoint(),
        'A'
    );
    assert!((0..3).all(|y| {
        list.get_cell(Point::Active(Coordinate { x: 0, y }))
            .unwrap()
            .cell
            .is_empty()
    }));
}

#[test]
fn last_nonempty_active_row_uses_one_reverse_walk() {
    let small = prompt_selection_work(64);
    let large = prompt_selection_work(128);
    eprintln!(
        "prompt selection N64={} N128={}; repeated N64={} N128={}",
        small.0, large.0, small.1, large.1
    );
    assert!(large.0 < small.0 * 3);
    assert!(large.1 > small.1 * 3);
}

/// Measure real arena reads against repeated active-origin lookup on the same rows.
fn prompt_selection_work(rows: u16) -> (usize, usize) {
    let mut list = screen(5, rows);
    for y in (16..rows).step_by(16) {
        let pin = list.pin_at_row(usize::from(y), 0).unwrap();
        list.split(pin).unwrap();
    }
    let first = list.pin(Point::Active(Coordinate::default())).unwrap();
    first
        .row_and_cell_mut(&mut list)
        .unwrap()
        .1
        .set_content(crate::page::cell::Content::Codepoint('$'));
    list.nodes.queries.set(0);
    let (selected, count) = list.last_nonempty_active_row().unwrap();
    assert_eq!((selected.node, selected.y), (first.node, first.y));
    assert_eq!(count, 1);
    let monotone = list.nodes.queries.get();
    list.nodes.queries.set(0);
    for y in (0..rows).rev() {
        let pin = list
            .pin(Point::Active(Coordinate {
                x: 0,
                y: u32::from(y),
            }))
            .unwrap();
        if pin
            .cells(&list, crate::page_list::CellSubset::All)
            .unwrap()
            .iter()
            .any(|cell| !cell.is_empty())
        {
            assert_eq!((pin.node, pin.y), (first.node, first.y));
            break;
        }
    }
    (monotone, list.nodes.queries.get())
}
