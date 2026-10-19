//! Screen resizing through real page-list storage and tracked cursor locations.

use super::screens::{contents, small_screen, write_text};
use maestro_terminal::{
    Coordinate, Point, Screen, osc::semantic_prompt::Redraw, point::Tag, screen::Resize,
};
use std::num::NonZeroU16;

/// Apply the source's explicit dimensions and default prompt policy.
fn resize(s: &mut Screen, cols: u16, rows: u16, reflow: bool) {
    s.resize(Resize {
        cols: NonZeroU16::new(cols).unwrap(),
        rows: NonZeroU16::new(rows).unwrap(),
        reflow,
        prompt_redraw: Redraw::False,
    })
    .unwrap();
}

#[test]
fn screen_resize_no_reflow_more_rows() {
    let mut s = small_screen(10, 3, 0);
    let text = "1ABCD\n2EFGH\n3IJKL";
    write_text(&mut s, text);
    resize(&mut s, 10, 10, false);
    assert_eq!(contents(&s, Tag::Viewport), text);
}

#[test]
fn screen_resize_no_reflow_less_rows() {
    let mut s = small_screen(10, 3, 0);
    write_text(&mut s, "1ABCD\n2EFGH\n3IJKL");
    assert_eq!((s.cursor.x, s.cursor.y), (5, 2));
    resize(&mut s, 10, 2, false);
    assert_eq!((s.cursor.x, s.cursor.y), (5, 1));
    assert_eq!(contents(&s, Tag::Viewport), "2EFGH\n3IJKL");
}

/// Background-only trailing cells remain blank for height trimming.
fn background_trimming(cols: u16, rows: u16) {
    let mut s = small_screen(10, 3, 0);
    write_text(&mut s, "1ABCD");
    for y in 1..3 {
        let pin = s.pages.pin(Point::Active(Coordinate { x: 0, y })).unwrap();
        pin.row_and_cell_mut(&mut s.pages).unwrap().1.set_content(
            maestro_terminal::page::cell::Content::BgColorRgb(maestro_terminal::color::Rgb {
                r: 255,
                g: 0,
                b: 0,
            }),
        );
    }
    let cursor = (s.cursor.x, s.cursor.y);
    resize(&mut s, cols, rows, false);
    assert_eq!((s.cursor.x, s.cursor.y), cursor);
    assert_eq!(contents(&s, Tag::Viewport), "1ABCD");
}

#[test]
fn screen_resize_no_reflow_less_rows_trims_blank_lines() {
    background_trimming(6, 2);
}

#[test]
fn screen_resize_no_reflow_more_rows_trims_blank_lines() {
    background_trimming(10, 7);
}

#[test]
fn screen_resize_no_reflow_more_cols() {
    let mut s = small_screen(10, 3, 0);
    let text = "1ABCD\n2EFGH\n3IJKL";
    write_text(&mut s, text);
    resize(&mut s, 20, 3, false);
    assert_eq!(contents(&s, Tag::Viewport), text);
}

#[test]
fn screen_resize_no_reflow_less_cols() {
    let mut s = small_screen(10, 3, 0);
    write_text(&mut s, "1ABCD\n2EFGH\n3IJKL");
    resize(&mut s, 4, 3, false);
    assert_eq!(contents(&s, Tag::Viewport), "1ABC\n2EFG\n3IJK");
}

#[test]
fn screen_resize_no_reflow_more_rows_with_scrollback_cursor_end() {
    let mut s = small_screen(7, 3, 2);
    let text = "1ABCD\n2EFGH\n3IJKL\n4ABCD\n5EFGH";
    write_text(&mut s, text);
    resize(&mut s, 7, 10, false);
    assert_eq!(contents(&s, Tag::Viewport), text);
}

#[test]
fn screen_resize_no_reflow_less_rows_with_scrollback() {
    let mut s = small_screen(7, 3, 2);
    write_text(&mut s, "1ABCD\n2EFGH\n3IJKL\n4ABCD\n5EFGH");
    resize(&mut s, 7, 2, false);
    assert_eq!(contents(&s, Tag::Viewport), "4ABCD\n5EFGH");
}

#[test]
fn screen_resize_no_reflow_less_rows_with_empty_trailing() {
    let mut s = small_screen(5, 3, 5);
    write_text(&mut s, "1\n2\n3\n4\n5\n6\n7\n8");
    s.scroll_clear();
    s.cursor_absolute(0, 0);
    write_text(&mut s, "A\nB");
    let cursor = (s.cursor.x, s.cursor.y);
    resize(&mut s, 5, 2, false);
    assert_eq!((s.cursor.x, s.cursor.y), cursor);
    assert_eq!(contents(&s, Tag::Viewport), "A\nB");
}

#[test]
fn screen_resize_no_reflow_more_rows_with_soft_wrapping() {
    let mut s = small_screen(2, 3, 3);
    write_text(&mut s, "1A2B\n3C4E\n5F6G");
    assert_wraps(&s);
    resize(&mut s, 2, 10, false);
    assert_eq!(contents(&s, Tag::Viewport), "1A\n2B\n3C\n4E\n5F\n6G");
    assert_wraps(&s);
}

/// Verify soft-wrap metadata survives a height-only resize.
fn assert_wraps(s: &Screen) {
    for y in 0..6 {
        let cell = s
            .pages
            .get_cell(Point::Screen(Coordinate { x: 0, y }))
            .unwrap();
        assert_eq!(cell.row.wrap(), y % 2 == 0);
    }
}

/// Height-only growth preserves the cursor and both visible/all retained text.
fn grow_without_history(history: usize) {
    let mut s = small_screen(5, 3, history);
    let text = "1ABCD\n2EFGH\n3IJKL";
    write_text(&mut s, text);
    let cursor = (s.cursor.x, s.cursor.y);
    resize(&mut s, 5, 10, true);
    assert_eq!((s.cursor.x, s.cursor.y), cursor);
    assert_eq!(contents(&s, Tag::Viewport), text);
    assert_eq!(contents(&s, Tag::Screen), text);
}

#[test]
fn screen_resize_more_rows_no_scrollback() {
    grow_without_history(0);
}

#[test]
fn screen_resize_more_rows_with_empty_scrollback() {
    grow_without_history(10);
}

/// Observe the character under the actual active cursor.
fn cursor_char(s: &Screen) -> char {
    s.pages
        .get_cell(Point::Active(Coordinate {
            x: s.cursor.x,
            y: u32::from(s.cursor.y),
        }))
        .unwrap()
        .cell
        .codepoint()
}

#[test]
fn screen_resize_more_rows_with_populated_scrollback() {
    let mut s = small_screen(5, 3, 5);
    write_text(&mut s, "1ABCD\n2EFGH\n3IJKL\n4ABCD\n5EFGH");
    assert_eq!(contents(&s, Tag::Viewport), "3IJKL\n4ABCD\n5EFGH");
    s.cursor_absolute(0, 1);
    assert_eq!(cursor_char(&s), '4');
    resize(&mut s, 5, 10, true);
    assert_eq!(cursor_char(&s), '4');
    assert_eq!(contents(&s, Tag::Viewport), "3IJKL\n4ABCD\n5EFGH");
}

#[test]
fn screen_resize_more_cols_no_reflow() {
    let mut s = small_screen(5, 3, 0);
    let text = "1ABCD\n2EFGH\n3IJKL";
    write_text(&mut s, text);
    let cursor = (s.cursor.x, s.cursor.y);
    resize(&mut s, 10, 3, true);
    assert_eq!((s.cursor.x, s.cursor.y), cursor);
    assert_eq!(contents(&s, Tag::Viewport), text);
    assert_eq!(contents(&s, Tag::Screen), text);
}

#[test]
fn screen_resize_more_cols_perfect_split() {
    let mut s = small_screen(5, 3, 0);
    write_text(&mut s, "1ABCD2EFGH3IJKL");
    resize(&mut s, 10, 3, true);
    assert_eq!(contents(&s, Tag::Screen), "1ABCD2EFGH\n3IJKL");
}

/// Width changes keep the active cursor while a historical viewport is pinned.
fn pinned_view(cols: u16) {
    let mut s = small_screen(5, 3, 5);
    let text = "1\n2\n3\n4\n5\n6\n7\n8";
    write_text(&mut s, text);
    assert_eq!((s.cursor.x, s.cursor.y), (1, 2));
    s.scroll(maestro_terminal::page_list::Scroll::DeltaRow(-4));
    assert_eq!(contents(&s, Tag::Viewport), "2\n3\n4");
    resize(&mut s, cols, 3, true);
    assert_eq!(contents(&s, Tag::Screen), text);
    assert_eq!(contents(&s, Tag::Active), "6\n7\n8");
    assert_eq!((s.cursor.x, s.cursor.y), (1, 2));
}

#[test]
fn screen_resize_no_reflow_more_cols_with_scrollback_scrolled_up() {
    pinned_view(8);
}

#[test]
fn screen_resize_no_reflow_less_cols_with_scrollback_scrolled_up() {
    pinned_view(4);
}

#[test]
fn screen_resize_more_cols_no_reflow_preserves_semantic_prompt() {
    let mut s = small_screen(5, 3, 0);
    let text = "1ABCD\n2EFGH\n3IJKL";
    write_text(&mut s, text);
    let pin = s
        .pages
        .pin(Point::Active(Coordinate { x: 0, y: 1 }))
        .unwrap();
    pin.row_and_cell_mut(&mut s.pages)
        .unwrap()
        .0
        .set_semantic_prompt(maestro_terminal::page::row::SemanticPrompt::Prompt);
    resize(&mut s, 10, 3, false);
    assert_eq!(contents(&s, Tag::Viewport), text);
    assert_eq!(contents(&s, Tag::Screen), text);
    for y in 0..3 {
        let role = s
            .pages
            .get_cell(Point::Active(Coordinate { x: 0, y }))
            .unwrap()
            .row
            .semantic_prompt();
        assert_eq!(
            role,
            if y == 1 {
                maestro_terminal::page::row::SemanticPrompt::Prompt
            } else {
                maestro_terminal::page::row::SemanticPrompt::None
            }
        );
    }
}

#[test]
fn screen_resize_more_cols_with_reflow_that_fits_full_width() {
    let mut s = small_screen(5, 3, 0);
    let text = "1ABCD2EFGH\n3IJKL";
    write_text(&mut s, text);
    assert_eq!(contents(&s, Tag::Viewport), "1ABCD\n2EFGH\n3IJKL");
    s.cursor_absolute(0, 1);
    assert_eq!(cursor_char(&s), '2');
    resize(&mut s, 10, 3, true);
    assert_eq!(contents(&s, Tag::Viewport), text);
    assert_eq!((s.cursor.x, s.cursor.y), (5, 0));
}

#[test]
fn screen_resize_more_cols_with_reflow_that_ends_in_newline() {
    let mut s = small_screen(6, 3, 0);
    let text = "1ABCD2EFGH\n3IJKL";
    write_text(&mut s, text);
    assert_eq!(contents(&s, Tag::Viewport), "1ABCD2\nEFGH\n3IJKL");
    s.cursor_absolute(0, 2);
    assert_eq!(cursor_char(&s), '3');
    resize(&mut s, 10, 3, true);
    assert_eq!(contents(&s, Tag::Viewport), text);
    assert_eq!(cursor_char(&s), '3');
}

#[test]
fn screen_resize_more_cols_with_reflow_that_forces_more_wrapping() {
    let mut s = small_screen(5, 3, 0);
    write_text(&mut s, "1ABCD2EFGH\n3IJKL");
    s.cursor_absolute(0, 1);
    assert_eq!(cursor_char(&s), '2');
    assert_eq!(contents(&s, Tag::Viewport), "1ABCD\n2EFGH\n3IJKL");
    resize(&mut s, 7, 3, true);
    assert_eq!(contents(&s, Tag::Viewport), "1ABCD2E\nFGH\n3IJKL");
    assert_eq!((s.cursor.x, s.cursor.y), (5, 0));
}

#[test]
fn screen_resize_more_cols_with_reflow_that_unwraps_multiple_times() {
    let mut s = small_screen(5, 3, 0);
    let text = "1ABCD2EFGH3IJKL";
    write_text(&mut s, text);
    s.cursor_absolute(0, 2);
    assert_eq!(cursor_char(&s), '3');
    assert_eq!(contents(&s, Tag::Viewport), "1ABCD\n2EFGH\n3IJKL");
    resize(&mut s, 15, 3, true);
    assert_eq!(contents(&s, Tag::Viewport), text);
    assert_eq!((s.cursor.x, s.cursor.y), (10, 0));
}

#[test]
fn screen_resize_more_cols_with_populated_scrollback() {
    let mut s = small_screen(5, 3, 5);
    write_text(&mut s, "1ABCD\n2EFGH\n3IJKL\n4ABCD5EFGH");
    assert_eq!(contents(&s, Tag::Viewport), "3IJKL\n4ABCD\n5EFGH");
    s.cursor_absolute(0, 2);
    assert_eq!(cursor_char(&s), '5');
    resize(&mut s, 10, 3, true);
    assert_eq!(contents(&s, Tag::Viewport), "2EFGH\n3IJKL\n4ABCD5EFGH");
    assert_eq!(cursor_char(&s), '5');
}

/// Mark fixture prompt rows through the existing row record, not a command parser.
fn mark_prompt(s: &mut Screen, y: u32, role: maestro_terminal::page::row::SemanticPrompt) {
    let pin = s.pages.pin(Point::Active(Coordinate { x: 0, y })).unwrap();
    pin.row_and_cell_mut(&mut s.pages)
        .unwrap()
        .0
        .set_semantic_prompt(role);
}

/// Apply an explicit redraw request to the same real resize entry point.
fn redraw(s: &mut Screen, cols: u16, policy: Redraw) {
    s.resize(Resize {
        cols: NonZeroU16::new(cols).unwrap(),
        rows: NonZeroU16::new(s.pages.rows()).unwrap(),
        reflow: true,
        prompt_redraw: policy,
    })
    .unwrap();
}

#[test]
fn screen_resize_more_cols_with_cursor_at_prompt() {
    let mut s = small_screen(10, 3, 5);
    write_text(&mut s, "ABCDE\n> echo");
    mark_prompt(
        &mut s,
        1,
        maestro_terminal::page::row::SemanticPrompt::Prompt,
    );
    s.cursor.semantic_content = maestro_terminal::page::cell::SemanticContent::Input;
    assert_eq!(contents(&s, Tag::Viewport), "ABCDE\n> echo");
    redraw(&mut s, 20, Redraw::True);
    assert_eq!((s.cursor.x, s.cursor.y), (6, 1));
    assert_eq!(contents(&s, Tag::Viewport), "ABCDE");
}

#[test]
fn screen_resize_more_cols_with_cursor_not_at_prompt() {
    let mut s = small_screen(10, 3, 5);
    let text = "ABCDE\n> echo\noutput";
    write_text(&mut s, text);
    mark_prompt(
        &mut s,
        1,
        maestro_terminal::page::row::SemanticPrompt::Prompt,
    );
    assert_eq!(contents(&s, Tag::Viewport), text);
    redraw(&mut s, 20, Redraw::True);
    assert_eq!((s.cursor.x, s.cursor.y), (6, 2));
    assert_eq!(contents(&s, Tag::Viewport), text);
}

#[test]
fn screen_resize_with_prompt_redraw_last_clears_only_one_line() {
    let mut s = small_screen(10, 4, 5);
    write_text(&mut s, "ABCDE\n> hello\nworld");
    mark_prompt(
        &mut s,
        1,
        maestro_terminal::page::row::SemanticPrompt::Prompt,
    );
    mark_prompt(
        &mut s,
        2,
        maestro_terminal::page::row::SemanticPrompt::PromptContinuation,
    );
    s.cursor.semantic_content = maestro_terminal::page::cell::SemanticContent::Input;
    assert_eq!(contents(&s, Tag::Viewport), "ABCDE\n> hello\nworld");
    redraw(&mut s, 20, Redraw::Last);
    assert_eq!(contents(&s, Tag::Viewport), "ABCDE\n> hello");
}

#[test]
fn screen_resize_with_prompt_redraw_last_multiline_prompt_clears_only_last_line() {
    let mut s = small_screen(20, 5, 5);
    write_text(&mut s, "line1\nline2\nline3");
    mark_prompt(
        &mut s,
        0,
        maestro_terminal::page::row::SemanticPrompt::Prompt,
    );
    for y in 1..3 {
        mark_prompt(
            &mut s,
            y,
            maestro_terminal::page::row::SemanticPrompt::PromptContinuation,
        );
    }
    s.cursor.semantic_content = maestro_terminal::page::cell::SemanticContent::Prompt;
    assert_eq!(contents(&s, Tag::Viewport), "line1\nline2\nline3");
    redraw(&mut s, 30, Redraw::Last);
    assert_eq!(contents(&s, Tag::Viewport), "line1\nline2");
}

#[test]
fn screen_prompt_redraw_is_explicit_and_preserves_output() {
    use maestro_terminal::page::{cell::SemanticContent, row::SemanticPrompt};
    let cases = [
        SemanticContent::Output,
        SemanticContent::Prompt,
        SemanticContent::Input,
    ]
    .into_iter()
    .flat_map(|semantic| {
        [Redraw::False, Redraw::True, Redraw::Last].map(|policy| (semantic, policy))
    });
    for (semantic, policy) in cases {
        for (found, cross_page) in [(false, false), (false, true), (true, false), (true, true)] {
            let mut s = small_screen(10, 5, 100_000);
            write_text(&mut s, "OUTPUT\nPROMPT\nCONT\nINPUT");
            if found {
                mark_prompt(&mut s, 1, SemanticPrompt::Prompt);
                mark_prompt(&mut s, 2, SemanticPrompt::PromptContinuation);
            }
            if cross_page {
                let p = s
                    .pages
                    .pin(Point::Active(Coordinate { x: 0, y: 2 }))
                    .unwrap();
                s.pages.split(p).unwrap();
                s.cursor_reload();
            }
            s.cursor.semantic_content = semantic;
            let rows = s.pages.total_rows();
            redraw(&mut s, 20, policy);
            let expected = if semantic == SemanticContent::Output || policy == Redraw::False {
                "OUTPUT\nPROMPT\nCONT\nINPUT"
            } else if policy == Redraw::Last {
                "OUTPUT\nPROMPT\nCONT"
            } else if found {
                "OUTPUT"
            } else {
                "OUTPUT\nPROMPT\nCONT\nINPUT"
            };
            assert_eq!(
                contents(&s, Tag::Viewport),
                expected,
                "{semantic:?} {policy:?} found={found} pages={cross_page}"
            );
            assert_eq!((s.cursor.x, s.cursor.y), (5, 3));
            assert_eq!(s.pages.total_rows(), rows);
            assert_eq!(s.pages.count_tracked_pins(), 2);
        }
    }
}

#[test]
fn screen_saved_cursor_outside_active_resets_to_origin() {
    let mut s = small_screen(5, 3, 100_000);
    write_text(&mut s, "FIRST\nMID\nLAST");
    s.charset.gl = maestro_terminal::CharsetSlot::G3;
    let style = maestro_terminal::style::Style {
        bg_color: maestro_terminal::style::Color::Palette(7),
        ..maestro_terminal::style::Style::default()
    };
    s.saved_cursor = Some(maestro_terminal::screen::SavedCursor {
        x: 4,
        y: 0,
        style,
        protected: true,
        pending_wrap: true,
        origin: true,
        charset: s.charset,
    });
    resize(&mut s, 5, 1, true);
    let saved = s.saved_cursor.unwrap();
    assert_eq!((saved.x, saved.y, saved.pending_wrap), (0, 0, false));
    assert_eq!(saved.style, style);
    assert_eq!(saved.charset, s.charset);
    assert!(saved.protected && saved.origin);
    assert_eq!(s.pages.count_tracked_pins(), 2);
    s.saved_cursor = None;
    resize(&mut s, 6, 2, true);
    assert!(s.saved_cursor.is_none());
}

#[test]
fn screen_resize_more_cols_bounded_scrollback_keeps_viewport_valid() {
    let mut s = small_screen(2, 10, 10_000);
    for _ in 0..30 {
        s.pages.grow();
    }
    s.cursor_reload();
    assert_eq!(s.pages.scrollbar().total, 40);
    for y in 0..40 {
        for x in 0..2 {
            let pin = s.pages.pin(Point::Screen(Coordinate { x, y })).unwrap();
            let (row, cell) = pin.row_and_cell_mut(&mut s.pages).unwrap();
            row.set_wrap(y % 2 == 0);
            row.set_wrap_continuation(y % 2 != 0);
            *cell = maestro_terminal::page::Cell::init('A');
        }
    }
    s.pages.scroll(maestro_terminal::page_list::Scroll::Row(28));
    assert!(!s.viewport_is_bottom());
    assert!(s.pages.get_bottom_right(Tag::Viewport).is_some());
    resize(&mut s, 4, 10, true);
    assert_eq!(s.pages.cols(), 4);
    assert!(s.pages.scrollbar().total < 40);
    assert!(s.viewport_is_bottom());
    assert!(s.pages.get_bottom_right(Tag::Viewport).is_some());
}

#[test]
fn screen_resize_more_cols_with_reflow() {
    let mut s = small_screen(2, 3, 5);
    let text = "1ABC\n2DEF\n3ABC\n4DEF";
    write_text(&mut s, text);
    s.cursor_absolute(0, 2);
    assert_eq!(cursor_char(&s), 'E');
    assert_eq!(contents(&s, Tag::Viewport), "BC\n4D\nEF");
    resize(&mut s, 7, 3, true);
    assert_eq!(contents(&s, Tag::Screen), text);
    assert_eq!((s.cursor.x, s.cursor.y), (2, 2));
}

#[test]
fn screen_resize_more_rows_and_cols_with_wrapping() {
    let mut s = small_screen(2, 4, 0);
    let text = "1A2B\n3C4D";
    write_text(&mut s, text);
    assert_eq!(contents(&s, Tag::Viewport), "1A\n2B\n3C\n4D");
    resize(&mut s, 5, 10, true);
    assert_eq!((s.cursor.x, s.cursor.y), (3, 1));
    assert_eq!(contents(&s, Tag::Viewport), text);
    assert_eq!(contents(&s, Tag::Screen), text);
}

#[test]
fn screen_resize_less_rows_no_scrollback() {
    let mut s = small_screen(5, 3, 0);
    write_text(&mut s, "1ABCD\n2EFGH\n3IJKL");
    s.cursor_absolute(0, 0);
    resize(&mut s, 5, 1, true);
    assert_eq!((s.cursor.x, s.cursor.y), (0, 0));
    assert_eq!(contents(&s, Tag::Viewport), "3IJKL");
    assert_eq!(contents(&s, Tag::Screen), "3IJKL");
}

#[test]
fn screen_resize_less_rows_moving_cursor() {
    let mut s = small_screen(5, 3, 0);
    write_text(&mut s, "1ABCD\n2EFGH\n3IJKL");
    s.cursor_absolute(1, 2);
    assert_eq!(cursor_char(&s), 'I');
    resize(&mut s, 5, 1, true);
    assert_eq!((s.cursor.x, s.cursor.y), (1, 0));
    assert_eq!(contents(&s, Tag::Viewport), "3IJKL");
    assert_eq!(contents(&s, Tag::Screen), "3IJKL");
}

#[test]
fn screen_resize_less_rows_with_empty_scrollback() {
    let mut s = small_screen(5, 3, 10);
    let text = "1ABCD\n2EFGH\n3IJKL";
    write_text(&mut s, text);
    resize(&mut s, 5, 1, true);
    assert_eq!(contents(&s, Tag::Screen), text);
    assert_eq!(contents(&s, Tag::Viewport), "3IJKL");
}

#[test]
fn screen_resize_less_rows_with_populated_scrollback() {
    let mut s = small_screen(5, 3, 5);
    let text = "1ABCD\n2EFGH\n3IJKL\n4ABCD\n5EFGH";
    write_text(&mut s, text);
    assert_eq!(contents(&s, Tag::Viewport), "3IJKL\n4ABCD\n5EFGH");
    resize(&mut s, 5, 1, true);
    assert_eq!(contents(&s, Tag::Screen), text);
    assert_eq!(contents(&s, Tag::Viewport), "5EFGH");
}

#[test]
fn screen_resize_less_rows_with_full_scrollback() {
    let mut s = small_screen(5, 3, 3);
    let text = "00000\n1ABCD\n2EFGH\n3IJKL\n4ABCD\n5EFGH";
    write_text(&mut s, text);
    assert_eq!(contents(&s, Tag::Viewport), "3IJKL\n4ABCD\n5EFGH");
    assert_eq!((s.cursor.x, s.cursor.y), (4, 2));
    resize(&mut s, 5, 2, true);
    assert_eq!((s.cursor.x, s.cursor.y), (4, 1));
    assert_eq!(contents(&s, Tag::Screen), text);
    assert_eq!(contents(&s, Tag::Viewport), "4ABCD\n5EFGH");
}

#[test]
fn screen_resize_less_cols_no_reflow() {
    let mut s = small_screen(5, 3, 0);
    let text = "1AB\n2EF\n3IJ";
    write_text(&mut s, text);
    s.cursor_absolute(0, 0);
    resize(&mut s, 3, 3, true);
    assert_eq!((s.cursor.x, s.cursor.y), (0, 0));
    assert_eq!(contents(&s, Tag::Viewport), text);
    assert_eq!(contents(&s, Tag::Screen), text);
}

#[test]
fn screen_resize_less_cols_with_reflow_but_row_space() {
    let mut s = small_screen(5, 3, 1);
    write_text(&mut s, "1ABCD");
    s.cursor_absolute(4, 0);
    assert_eq!(cursor_char(&s), 'D');
    resize(&mut s, 3, 3, true);
    assert_eq!(contents(&s, Tag::Viewport), "1AB\nCD");
    assert_eq!(contents(&s, Tag::Screen), "1AB\nCD");
    assert_eq!((s.cursor.x, s.cursor.y), (1, 1));
}

/// A width reduction retains only the permitted history while reflowing all rows.
fn trimmed_rows(history: usize) {
    let mut s = small_screen(5, 3, history);
    write_text(&mut s, "3IJKL\n4ABCD\n5EFGH");
    resize(&mut s, 3, 3, true);
    assert_eq!(contents(&s, Tag::Viewport), "CD\n5EF\nGH");
    assert_eq!(
        contents(&s, Tag::Screen),
        if history == 0 {
            "CD\n5EF\nGH"
        } else {
            "3IJ\nKL\n4AB\nCD\n5EF\nGH"
        }
    );
}

#[test]
fn screen_resize_less_cols_with_reflow_with_trimmed_rows() {
    trimmed_rows(0);
}

#[test]
fn screen_resize_less_cols_with_reflow_with_trimmed_rows_and_scrollback() {
    trimmed_rows(1);
}

#[test]
fn screen_resize_less_cols_with_reflow_previously_wrapped() {
    let mut s = small_screen(5, 3, 0);
    write_text(&mut s, "3IJKL4ABCD5EFGH");
    assert_eq!(contents(&s, Tag::Screen), "3IJKL\n4ABCD\n5EFGH");
    resize(&mut s, 3, 3, true);
    assert_eq!(contents(&s, Tag::Screen), "ABC\nD5E\nFGH");
}

#[test]
fn screen_resize_less_cols_with_reflow_and_scrollback() {
    let mut s = small_screen(5, 3, 5);
    write_text(&mut s, "1A\n2B\n3C\n4D\n5E");
    s.cursor_absolute(1, 2);
    assert_eq!(cursor_char(&s), 'E');
    resize(&mut s, 3, 3, true);
    assert_eq!(contents(&s, Tag::Viewport), "3C\n4D\n5E");
    assert_eq!((s.cursor.x, s.cursor.y), (1, 2));
}

#[test]
fn screen_resize_less_cols_with_reflow_previously_wrapped_and_scrollback() {
    let mut s = small_screen(5, 3, 2);
    write_text(&mut s, "1ABCD2EFGH3IJKL4ABCD5EFGH");
    assert_eq!(contents(&s, Tag::Viewport), "3IJKL\n4ABCD\n5EFGH");
    s.cursor_absolute(4, 2);
    assert_eq!(cursor_char(&s), 'H');
    resize(&mut s, 3, 3, true);
    assert_eq!(contents(&s, Tag::Viewport), "CD5\nEFG\nH");
    assert_eq!(
        contents(&s, Tag::Screen),
        "1AB\nCD2\nEFG\nH3I\nJKL\n4AB\nCD5\nEFG\nH"
    );
    assert_eq!((s.cursor.x, s.cursor.y), (0, 2));
    assert_eq!(cursor_char(&s), 'H');
}

#[test]
fn screen_resize_less_cols_with_scrollback_keeps_cursor_row() {
    let mut s = small_screen(5, 3, 5);
    write_text(&mut s, "1A\n2B\n3C\n4D\n5E");
    s.scroll_clear();
    s.cursor_absolute(0, 0);
    resize(&mut s, 3, 3, true);
    assert_eq!(contents(&s, Tag::Viewport), "");
    assert_eq!((s.cursor.x, s.cursor.y), (0, 0));
}

#[test]
fn screen_resize_more_rows_less_cols_with_reflow_with_scrollback() {
    let mut s = small_screen(5, 3, 3);
    write_text(&mut s, "1ABCD\n2EFGH3IJKL\n4MNOP");
    assert_eq!(contents(&s, Tag::Screen), "1ABCD\n2EFGH\n3IJKL\n4MNOP");
    assert_eq!(contents(&s, Tag::Viewport), "2EFGH\n3IJKL\n4MNOP");
    resize(&mut s, 2, 10, true);
    assert_eq!(
        contents(&s, Tag::Viewport),
        "BC\nD\n2E\nFG\nH3\nIJ\nKL\n4M\nNO\nP"
    );
    assert_eq!(
        contents(&s, Tag::Screen),
        "1A\nBC\nD\n2E\nFG\nH3\nIJ\nKL\n4M\nNO\nP"
    );
}

#[test]
fn screen_resize_more_rows_then_shrink_again() {
    let mut s = small_screen(5, 3, 10);
    write_text(&mut s, "1ABC");
    for rows in [10, 3, 10] {
        resize(&mut s, 5, rows, true);
        assert_eq!(contents(&s, Tag::Viewport), "1ABC");
        assert_eq!(contents(&s, Tag::Screen), "1ABC");
    }
}

#[test]
fn screen_resize_less_cols_reflows_cursor_after_wrapped_text() {
    let mut s = small_screen(50, 7, 0);
    write_text(&mut s, &"a".repeat(30));
    assert_eq!((s.cursor.x, s.cursor.y), (30, 0));
    resize(&mut s, 25, 7, true);
    assert_eq!((s.cursor.x, s.cursor.y), (5, 1));
}

#[test]
fn screen_resize_less_cols_reflows_cursor_after_empty_cells() {
    let mut s = small_screen(10, 3, 0);
    write_text(&mut s, "abc");
    s.cursor_right(6);
    assert_eq!((s.cursor.x, s.cursor.y), (9, 0));
    resize(&mut s, 5, 3, true);
    assert_eq!((s.cursor.x, s.cursor.y), (4, 1));
}

/// Inspect a stored wide-character role and its scalar through the real screen.
fn wide_cell(s: &Screen, x: u16, y: u32, wide: maestro_terminal::page::cell::Wide, ch: char) {
    let cell = s
        .pages
        .get_cell(Point::Screen(Coordinate { x, y }))
        .unwrap();
    assert_eq!(cell.cell.wide(), wide);
    assert_eq!(cell.cell.codepoint(), ch);
}

#[test]
fn screen_resize_less_cols_to_eliminate_wide_char() {
    use maestro_terminal::page::cell::Wide;
    let mut s = small_screen(2, 1, 0);
    write_text(&mut s, "😀");
    assert_eq!(contents(&s, Tag::Screen), "😀");
    wide_cell(&s, 0, 0, Wide::Wide, '😀');
    resize(&mut s, 1, 1, true);
    assert_eq!(contents(&s, Tag::Screen), "");
    wide_cell(&s, 0, 0, Wide::Narrow, '\0');
}

#[test]
fn screen_resize_less_cols_to_wrap_wide_char() {
    use maestro_terminal::page::cell::Wide;
    let mut s = small_screen(3, 3, 0);
    write_text(&mut s, "x😀");
    assert_eq!(contents(&s, Tag::Screen), "x😀");
    wide_cell(&s, 1, 0, Wide::Wide, '😀');
    wide_cell(&s, 2, 0, Wide::SpacerTail, '\0');
    resize(&mut s, 2, 3, true);
    assert_eq!(contents(&s, Tag::Screen), "x\n😀");
    wide_cell(&s, 1, 0, Wide::SpacerHead, '\0');
    assert!(
        s.pages
            .get_cell(Point::Screen(Coordinate { x: 1, y: 0 }))
            .unwrap()
            .row
            .wrap()
    );
}

#[test]
fn screen_resize_less_cols_to_eliminate_wide_char_with_row_space() {
    use maestro_terminal::page::cell::Wide;
    let mut s = small_screen(2, 2, 0);
    write_text(&mut s, "😀");
    assert_eq!(contents(&s, Tag::Screen), "😀");
    wide_cell(&s, 0, 0, Wide::Wide, '😀');
    wide_cell(&s, 1, 0, Wide::SpacerTail, '\0');
    resize(&mut s, 1, 2, true);
    assert_eq!(contents(&s, Tag::Screen), "");
}

#[test]
fn screen_resize_more_cols_with_wide_spacer_head() {
    use maestro_terminal::page::cell::Wide;
    let mut s = small_screen(3, 2, 0);
    write_text(&mut s, "  😀");
    assert_eq!(contents(&s, Tag::Screen), "  \n😀");
    wide_cell(&s, 2, 0, Wide::SpacerHead, '\0');
    wide_cell(&s, 0, 1, Wide::Wide, '😀');
    wide_cell(&s, 1, 1, Wide::SpacerTail, '\0');
    resize(&mut s, 4, 2, true);
    assert_eq!(contents(&s, Tag::Screen), "  😀");
    wide_cell(&s, 2, 0, Wide::Wide, '😀');
    wide_cell(&s, 3, 0, Wide::SpacerTail, '\0');
}

#[test]
fn screen_resize_more_cols_with_wide_spacer_head_multiple_lines() {
    use maestro_terminal::page::cell::Wide;
    let mut s = small_screen(3, 3, 0);
    write_text(&mut s, "xxxyy😀");
    assert_eq!(contents(&s, Tag::Screen), "xxx\nyy\n😀");
    wide_cell(&s, 2, 1, Wide::SpacerHead, '\0');
    wide_cell(&s, 0, 2, Wide::Wide, '😀');
    wide_cell(&s, 1, 2, Wide::SpacerTail, '\0');
    resize(&mut s, 8, 2, true);
    assert_eq!(contents(&s, Tag::Screen), "xxxyy😀");
    wide_cell(&s, 5, 0, Wide::Wide, '😀');
    wide_cell(&s, 6, 0, Wide::SpacerTail, '\0');
}

#[test]
fn screen_resize_more_cols_requiring_a_wide_spacer_head() {
    use maestro_terminal::page::cell::Wide;
    let mut s = small_screen(2, 2, 0);
    write_text(&mut s, "xx😀");
    assert_eq!(contents(&s, Tag::Screen), "xx\n😀");
    wide_cell(&s, 0, 1, Wide::Wide, '😀');
    wide_cell(&s, 1, 1, Wide::SpacerTail, '\0');
    resize(&mut s, 3, 2, true);
    assert_eq!(contents(&s, Tag::Screen), "xx\n😀");
    wide_cell(&s, 2, 0, Wide::SpacerHead, '\0');
    wide_cell(&s, 0, 1, Wide::Wide, '😀');
    wide_cell(&s, 1, 1, Wide::SpacerTail, '\0');
}
