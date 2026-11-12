//! Retained coordinates while soft lines join or split.

use super::reflow::{flags, reflow};
use super::resize::{active, integrity, nz, screen, text, write};
use super::*;
use crate::page::row::SemanticPrompt;

/// Two soft-linked rows with an optional third continuation.
fn wrapped_screen(third: bool) -> PageList {
    let mut list = PageList::init(nz(2), nz(4), Some(0));
    assert_eq!(list.total_pages(), 1);
    for y in 0..if third { 3 } else { 2 } {
        write(&mut list, y, "\0\u{1}");
        let page = list.page_mut(list.first).unwrap();
        let row = page.get_row_mut(u16::try_from(y).unwrap());
        row.set_wrap(y == 0 || (third && y == 1));
        row.set_wrap_continuation(y > 0);
    }
    list
}

#[test]
fn history_resize_reflow_more_cols_cursor_in_wrapped_row() {
    let mut list = wrapped_screen(false);
    let pin = list.track_pin(list.pin_at_row(1, 1).unwrap()).unwrap();
    reflow(&mut list, 4);
    assert_eq!((list.cols(), list.total_rows()), (4, 4));
    assert_eq!(active(&list, pin), Some(Coordinate { x: 3, y: 0 }));
    assert_eq!(text(&list, 0), "\0\u{1}\0\u{1}");
    assert_eq!(flags(&list, 0), (false, false));
    integrity(&list);
}

#[test]
fn history_resize_reflow_more_cols_cursor_in_not_wrapped_row() {
    let mut list = wrapped_screen(false);
    let pin = list.track_pin(list.pin_at_row(0, 1).unwrap()).unwrap();
    reflow(&mut list, 4);
    assert_eq!((list.cols(), list.total_rows()), (4, 4));
    assert_eq!(active(&list, pin), Some(Coordinate { x: 1, y: 0 }));
    assert_eq!(text(&list, 0), "\0\u{1}\0\u{1}");
    assert_eq!(text(&list, 1), "\0\0\0\0");
    integrity(&list);
}

#[test]
fn history_resize_reflow_more_cols_cursor_in_wrapped_row_that_isn_t_unwrapped() {
    let mut list = wrapped_screen(true);
    let pin = list.track_pin(list.pin_at_row(2, 1).unwrap()).unwrap();
    reflow(&mut list, 4);
    assert_eq!((list.cols(), list.total_rows()), (4, 4));
    assert_eq!(active(&list, pin), Some(Coordinate { x: 1, y: 1 }));
    assert_eq!(text(&list, 0), "\0\u{1}\0\u{1}");
    assert_eq!(text(&list, 1), "\0\u{1}\0\0");
    assert_eq!(flags(&list, 0), (true, false));
    assert_eq!(flags(&list, 1), (false, true));
    integrity(&list);
}

#[test]
fn history_resize_reflow_more_cols_no_reflow_preserves_semantic_prompt() {
    let mut list = PageList::init(nz(2), nz(4), Some(0));
    assert_eq!(list.total_pages(), 1);
    list.page_mut(list.first)
        .unwrap()
        .get_row_mut(1)
        .set_semantic_prompt(SemanticPrompt::Prompt);
    reflow(&mut list, 4);
    assert_eq!((list.cols(), list.total_rows()), (4, 4));
    assert_eq!(
        list.page(list.first).unwrap().get_row(1).semantic_prompt(),
        SemanticPrompt::Prompt
    );
    assert_eq!(
        list.page(list.first).unwrap().get_row(0).semantic_prompt(),
        SemanticPrompt::None
    );
    integrity(&list);
}

/// Two full hard lines that each need splitting.
fn full_rows() -> PageList {
    let mut list = screen(4, 2);
    assert_eq!(list.total_pages(), 1);
    for y in 0..2 {
        write(&mut list, y, "\0\u{1}\u{2}\u{3}");
    }
    list
}

#[test]
fn history_resize_reflow_less_cols_wrapped_rows() {
    let mut list = full_rows();
    reflow(&mut list, 2);
    assert_eq!((list.cols(), list.total_rows()), (2, 4));
    assert_eq!(
        list.point_from_pin(
            crate::point::Tag::Screen,
            list.get_top_left(crate::point::Tag::Active)
        ),
        Some(Point::Screen(Coordinate { x: 0, y: 2 }))
    );
    for y in 0..4 {
        assert_eq!(
            text(&list, y),
            if y % 2 == 0 { "\0\u{1}" } else { "\u{2}\u{3}" }
        );
        assert_eq!(flags(&list, y), (y % 2 == 0, y % 2 == 1));
    }
    integrity(&list);
}

#[test]
fn history_resize_reflow_less_cols_cursor_in_wrapped_row() {
    let mut list = full_rows();
    let pin = list.track_pin(list.pin_at_row(1, 2).unwrap()).unwrap();
    reflow(&mut list, 2);
    assert_eq!((list.cols(), list.total_rows()), (2, 4));
    assert_eq!(active(&list, pin), Some(Coordinate { x: 0, y: 1 }));
    assert_eq!(
        list.tracked_pin(pin)
            .unwrap()
            .row_and_cell(&list)
            .unwrap()
            .1
            .codepoint(),
        '\u{2}'
    );
    integrity(&list);
}

#[test]
fn history_resize_reflow_less_cols_cursor_goes_to_scrollback() {
    let mut list = full_rows();
    let pin = list.track_pin(list.pin_at_row(0, 2).unwrap()).unwrap();
    reflow(&mut list, 2);
    assert_eq!((list.cols(), list.total_rows()), (2, 4));
    assert_eq!(active(&list, pin), None);
    assert_eq!(
        list.point_from_pin(crate::point::Tag::Screen, *list.tracked_pin(pin).unwrap()),
        Some(Point::Screen(Coordinate { x: 0, y: 1 }))
    );
    assert_eq!(
        list.tracked_pin(pin)
            .unwrap()
            .row_and_cell(&list)
            .unwrap()
            .1
            .codepoint(),
        '\u{2}'
    );
    integrity(&list);
}

#[test]
fn history_resize_reflow_less_cols_cursor_in_unchanged_row() {
    let mut list = screen(4, 2);
    assert_eq!(list.total_pages(), 1);
    for y in 0..2 {
        write(&mut list, y, "\0\u{1}");
    }
    let pin = list.track_pin(list.pin_at_row(0, 1).unwrap()).unwrap();
    reflow(&mut list, 2);
    assert_eq!((list.cols(), list.total_rows()), (2, 2));
    assert_eq!(active(&list, pin), Some(Coordinate { x: 1, y: 0 }));
    assert_eq!(text(&list, 0), "\0\u{1}");
    assert_eq!(text(&list, 1), "\0\u{1}");
    integrity(&list);
}

#[test]
fn history_resize_reflow_less_cols_blank_lines_between_no_scrollback() {
    let mut list = PageList::init(nz(5), nz(3), Some(0));
    assert_eq!(list.total_pages(), 1);
    write(&mut list, 0, "A");
    write(&mut list, 2, "C");
    reflow(&mut list, 2);
    assert_eq!((list.cols(), list.total_rows()), (2, 3));
    for (y, expected) in ["A\0", "\0\0", "C\0"].into_iter().enumerate() {
        assert_eq!(text(&list, y), expected);
        assert_eq!(flags(&list, y), (false, false));
    }
    integrity(&list);
}

#[test]
fn history_resize_reflow_less_cols_cursor_not_on_last_line_preserves_location() {
    let mut list = PageList::init(nz(5), nz(5), Some(1));
    assert_eq!(list.total_pages(), 1);
    for y in 0..5 {
        write(&mut list, y, "\0\u{1}");
    }
    for _ in 0..5 {
        list.grow();
    }
    assert_eq!(list.total_rows(), 10);
    let pin = list.track_pin(list.pin_at_row(5, 0).unwrap()).unwrap();
    list.resize(Resize {
        cols: Some(nz(4)),
        cursor: Some(Cursor {
            x: 1,
            y: 1,
            pin: None,
        }),
        ..Resize::default()
    })
    .unwrap();
    assert_eq!((list.cols(), list.total_rows()), (4, 10));
    assert_eq!(active(&list, pin), Some(Coordinate { x: 0, y: 0 }));
    assert_eq!(text(&list, 4), "\0\u{1}\0\0");
    assert_eq!(text(&list, 9), "\0\0\0\0");
    integrity(&list);
}
