//! Soft-line rewrapping through the actual owned destination pages.

use super::resize::{active, integrity, nz, screen, screen_origin, text, write};
use super::*;
use crate::page::{CellPosition, row::SemanticPrompt};

/// Change width with the default rewrapping policy.
pub(super) fn reflow(list: &mut PageList, cols: u16) {
    list.resize(Resize {
        cols: Some(nz(cols)),
        ..Resize::default()
    })
    .unwrap();
    assert_eq!(list.cols(), cols);
}

/// Read the wrap and continuation bits at an absolute row.
pub(super) fn flags(list: &PageList, row: usize) -> (bool, bool) {
    let pin = list.pin_at_row(row, 0).unwrap();
    let metadata = list.page(pin.node).unwrap().get_row(pin.y);
    (metadata.wrap(), metadata.wrap_continuation())
}

#[test]
fn history_resize_reflow_more_cols_no_wrapped_rows() {
    let mut list = screen(5, 3);
    assert_eq!(list.total_pages(), 1);
    for y in 0..3 {
        write(&mut list, y, "AAAAA");
    }
    let pin = list.track_pin(list.pin_at_row(1, 2).unwrap()).unwrap();
    reflow(&mut list, 10);
    assert_eq!(list.total_rows(), 3);
    for y in 0..3 {
        assert_eq!(text(&list, y), "AAAAA\0\0\0\0\0");
        assert_eq!(flags(&list, y), (false, false));
    }
    assert_eq!(active(&list, pin), Some(Coordinate { x: 2, y: 1 }));
    integrity(&list);
}

#[test]
fn history_resize_reflow_more_cols_wrapped_rows() {
    let mut list = screen(2, 4);
    assert_eq!(list.total_pages(), 1);
    for y in 0..4 {
        write(&mut list, y, "AA");
    }
    let page = list.page_mut(list.first).unwrap();
    for y in 0..4 {
        page.get_row_mut(y).set_wrap(y % 2 == 0);
        page.get_row_mut(y).set_wrap_continuation(y % 2 == 1);
    }
    let pin = list.track_pin(list.pin_at_row(1, 0).unwrap()).unwrap();
    reflow(&mut list, 4);
    assert_eq!(list.total_rows(), 4);
    assert_eq!(text(&list, 0), "AAAA");
    assert_eq!(text(&list, 1), "AAAA");
    assert_eq!(text(&list, 2), "\0\0\0\0");
    assert_eq!(flags(&list, 0), (false, false));
    screen_origin(&list, crate::point::Tag::Active, 0);
    assert_eq!(active(&list, pin), Some(Coordinate { x: 2, y: 0 }));
    integrity(&list);
}

#[test]
fn history_resize_reflow_less_cols_no_wrapped_rows() {
    let mut list = screen(10, 3);
    assert_eq!(list.total_pages(), 1);
    for y in 0..3 {
        write(&mut list, y, "\0\u{1}\u{2}\u{3}");
    }
    let pin = list.track_pin(list.pin_at_row(1, 3).unwrap()).unwrap();
    reflow(&mut list, 5);
    assert_eq!(list.total_rows(), 3);
    for y in 0..3 {
        assert_eq!(text(&list, y), "\0\u{1}\u{2}\u{3}\0");
    }
    assert_eq!(active(&list, pin), Some(Coordinate { x: 3, y: 1 }));
    integrity(&list);
}

#[test]
fn history_resize_reflow_less_cols_wrapped_rows_with_graphemes() {
    let mut list = screen(4, 2);
    assert_eq!(list.total_pages(), 1);
    for y in 0..2 {
        write(&mut list, y, "\0\u{1}\u{2}\u{3}");
    }
    let page = list.page_mut(list.first).unwrap();
    for y in 0..2 {
        page.set_graphemes(CellPosition { x: 2, y }, &['A'])
            .unwrap();
    }
    let pin = list.track_pin(list.pin_at_row(1, 2).unwrap()).unwrap();
    reflow(&mut list, 2);
    assert_eq!((list.total_rows(), list.history_rows()), (4, 2));
    screen_origin(&list, crate::point::Tag::Active, 2);
    for y in [0, 2] {
        assert_eq!(text(&list, y), "\0\u{1}");
        assert_eq!(flags(&list, y), (true, false));
    }
    for y in [1, 3] {
        assert_eq!(text(&list, y), "\u{2}\u{3}");
        assert_eq!(flags(&list, y), (false, true));
        let pin = list.pin_at_row(y, 0).unwrap();
        assert!(pin.row_and_cell(&list).unwrap().0.grapheme());
        assert_eq!(
            pin.grapheme(&list, CellPosition { x: pin.x, y: pin.y }),
            Some(&['A'][..])
        );
    }
    assert_eq!(active(&list, pin), Some(Coordinate { x: 0, y: 1 }));
    assert_eq!(list.total_pages(), 1);
    integrity(&list);
}

#[test]
fn history_resize_reflow_less_cols_no_reflow_preserves_semantic_prompt_on_first_line() {
    let mut list = screen(4, 4);
    assert_eq!(list.total_pages(), 1);
    list.page_mut(list.first)
        .unwrap()
        .get_row_mut(0)
        .set_semantic_prompt(SemanticPrompt::Prompt);
    reflow(&mut list, 2);
    assert_eq!(list.total_rows(), 4);
    assert_eq!(
        list.page(list.first).unwrap().get_row(0).semantic_prompt(),
        SemanticPrompt::Prompt
    );
    assert_eq!(text(&list, 1), "\0\0");
    assert_eq!(list.total_pages(), 1);
    integrity(&list);
}

#[test]
fn history_resize_reflow_less_cols_no_reflow_preserves_semantic_prompt() {
    let mut list = screen(4, 4);
    assert_eq!(list.total_pages(), 1);
    write(&mut list, 1, "\0\u{1}\u{2}\u{3}");
    list.page_mut(list.first)
        .unwrap()
        .get_row_mut(1)
        .set_semantic_prompt(SemanticPrompt::Prompt);
    reflow(&mut list, 2);
    assert_eq!(list.total_rows(), 4);
    let page = list.page(list.first).unwrap();
    assert!(page.get_row(1).wrap());
    assert_eq!(page.get_row(1).semantic_prompt(), SemanticPrompt::Prompt);
    assert_eq!(page.get_row(2).semantic_prompt(), SemanticPrompt::Prompt);
    assert_eq!(text(&list, 2), "\u{2}\u{3}");
    assert_eq!(list.total_pages(), 1);
    integrity(&list);
}

/// Noncursor retained pins in trailing blanks clamp without forcing new wraps.
fn blank_pin(x: u16, expected: u16) {
    let mut list = screen(6, 2);
    assert_eq!(list.total_pages(), 1);
    for y in 0..2 {
        write(&mut list, y, "\0\u{1}");
    }
    let pin = list.track_pin(list.pin_at_row(0, x).unwrap()).unwrap();
    reflow(&mut list, 4);
    assert_eq!(list.total_rows(), 2);
    assert_eq!(active(&list, pin), Some(Coordinate { x: expected, y: 0 }));
    assert_eq!(text(&list, 1), "\0\u{1}\0\0");
    assert_eq!(list.total_pages(), 1);
    integrity(&list);
}

#[test]
fn history_resize_reflow_less_cols_cursor_in_blank_cell() {
    blank_pin(2, 2);
}

#[test]
fn history_resize_reflow_less_cols_cursor_in_final_blank_cell() {
    blank_pin(3, 3);
}

#[test]
fn history_resize_reflow_less_cols_cursor_in_wrapped_blank_cell() {
    blank_pin(5, 3);
}

#[test]
fn history_resize_reflow_less_cols_blank_lines() {
    let mut list = screen(4, 3);
    assert_eq!(list.total_pages(), 1);
    write(&mut list, 0, "\0\u{1}\u{2}\u{3}");
    reflow(&mut list, 2);
    assert_eq!(list.total_rows(), 3);
    assert_eq!(text(&list, 0), "\0\u{1}");
    assert_eq!(text(&list, 1), "\u{2}\u{3}");
    assert_eq!(text(&list, 2), "\0\0");
    assert_eq!(flags(&list, 0), (true, false));
    assert_eq!(flags(&list, 1), (false, true));
    integrity(&list);
}

#[test]
fn history_resize_reflow_less_cols_blank_lines_between() {
    let mut list = screen(4, 3);
    assert_eq!(list.total_pages(), 1);
    for y in [0, 2] {
        write(&mut list, y, "\0\u{1}\u{2}\u{3}");
    }
    reflow(&mut list, 2);
    assert_eq!((list.total_rows(), list.history_rows()), (5, 2));
    assert_eq!(text(&list, 2), "\0\0");
    assert_eq!(text(&list, 3), "\0\u{1}");
    assert_eq!(text(&list, 4), "\u{2}\u{3}");
    assert_eq!(flags(&list, 2), (false, false));
    assert_eq!(flags(&list, 3), (true, false));
    assert_eq!(flags(&list, 4), (false, true));
    integrity(&list);
}

#[test]
fn history_resize_less_rows_and_cols_cursor_at_bottom() {
    let mut list = screen(80, 24);
    assert_eq!(list.total_pages(), 1);
    let pin = list.track_pin(list.pin_at_row(23, 0).unwrap()).unwrap();
    list.resize(Resize {
        cols: Some(nz(79)),
        rows: Some(nz(20)),
        cursor: Some(Cursor {
            x: 0,
            y: 23,
            pin: Some(pin),
        }),
        ..Resize::default()
    })
    .unwrap();
    assert_eq!((list.cols(), list.rows()), (79, 20));
    assert_eq!(active(&list, pin), Some(Coordinate { x: 0, y: 19 }));
    integrity(&list);
}

#[test]
fn history_resize_less_rows_and_cols_cursor_near_top_pushed_to_scrollback() {
    let mut list = screen(80, 24);
    assert_eq!(list.total_pages(), 1);
    let line: String = (0..80).map(|x| char::from(b'A' + x % 26)).collect();
    for y in 0..24 {
        write(&mut list, y, &line);
    }
    let pin = list.track_pin(list.pin_at_row(0, 0).unwrap()).unwrap();
    list.resize(Resize {
        cols: Some(nz(79)),
        rows: Some(nz(20)),
        cursor: Some(Cursor {
            x: 0,
            y: 0,
            pin: Some(pin),
        }),
        ..Resize::default()
    })
    .unwrap();
    assert_eq!((list.cols(), list.rows()), (79, 20));
    assert_eq!(active(&list, pin), None);
    assert!(list.pin_is_valid(*list.tracked_pin(pin).unwrap()));
    assert!(text(&list, 0).starts_with('A'));
    integrity(&list);
}

#[test]
fn history_resize_more_rows_and_cols_doesn_t_fit_in_single_std_page() {
    let mut list = screen(10, 10);
    assert_eq!(list.total_pages(), 1);
    assert!(PageList::initial_capacity(600).rows < 600);
    list.resize(Resize {
        cols: Some(nz(600)),
        rows: Some(nz(600)),
        ..Resize::default()
    })
    .unwrap();
    assert_eq!(
        (list.cols(), list.rows(), list.total_rows()),
        (600, 600, 600)
    );
    assert!(list.total_pages() > 1);
    assert_eq!(text(&list, 599).len(), 600);
    integrity(&list);
}

#[test]
fn history_resize_reflow_less_cols_to_eliminate_a_wide_char() {
    let mut list = screen(2, 1);
    assert_eq!(list.total_pages(), 1);
    let page = list.page_mut(list.first).unwrap();
    page.get_cells_mut(0)[0] = crate::page::cell::Cell::init('😀');
    page.get_cells_mut(0)[0].set_wide(crate::page::cell::Wide::Wide);
    page.get_cells_mut(0)[1].set_wide(crate::page::cell::Wide::SpacerTail);
    reflow(&mut list, 1);
    assert_eq!(list.total_rows(), 1);
    assert_eq!(text(&list, 0), "\0");
    assert_eq!(
        list.page(list.first).unwrap().get_cells(0)[0].wide(),
        crate::page::cell::Wide::Narrow
    );
    assert_eq!(list.total_pages(), 1);
    integrity(&list);
}

#[test]
fn history_resize_reflow_less_cols_to_wrap_a_wide_char() {
    let mut list = screen(3, 1);
    assert_eq!(list.total_pages(), 1);
    write(&mut list, 0, "x😀");
    let page = list.page_mut(list.first).unwrap();
    page.get_cells_mut(0)[1].set_wide(crate::page::cell::Wide::Wide);
    page.get_cells_mut(0)[2].set_wide(crate::page::cell::Wide::SpacerTail);
    reflow(&mut list, 2);
    assert_eq!(list.total_rows(), 2);
    assert_eq!(text(&list, 0), "x\0");
    assert_eq!(text(&list, 1), "😀\0");
    assert_eq!(flags(&list, 0), (true, false));
    assert_eq!(flags(&list, 1), (false, true));
    let page = list.page(list.first).unwrap();
    assert_eq!(page.get_cells(0)[0].wide(), crate::page::cell::Wide::Narrow);
    assert_eq!(
        page.get_cells(0)[1].wide(),
        crate::page::cell::Wide::SpacerHead
    );
    assert_eq!(page.get_cells(1)[0].wide(), crate::page::cell::Wide::Wide);
    assert_eq!(
        page.get_cells(1)[1].wide(),
        crate::page::cell::Wide::SpacerTail
    );
    assert_eq!(list.total_pages(), 1);
    integrity(&list);
}

#[test]
fn history_resize_reflow_invalidates_viewport_offset_cache() {
    let mut list = screen(2, 4);
    assert_eq!(list.total_pages(), 1);
    for _ in 0..20 {
        list.grow();
    }
    for y in 0..4 {
        write(&mut list, y, "AA");
        let pin = list.pin_at_row(y, 0).unwrap();
        let row = list.page_mut(pin.node).unwrap().get_row_mut(pin.y);
        row.set_wrap(y % 2 == 0);
        row.set_wrap_continuation(y % 2 == 1);
    }
    list.scroll(Scroll::Row(10));
    assert_eq!(list.viewport, Viewport::Pin);
    assert_eq!(
        list.scrollbar(),
        Scrollbar {
            total: 24,
            offset: 10,
            len: 4
        }
    );
    reflow(&mut list, 4);
    assert_eq!(list.cols(), 4);
    assert_eq!(
        list.scrollbar(),
        Scrollbar {
            total: list.total_rows(),
            offset: 5,
            len: 4
        }
    );
    integrity(&list);
}
