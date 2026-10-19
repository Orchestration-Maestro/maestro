//! Screen state through the public terminal interface.

use maestro_terminal::{
    Cell, Charset, CharsetSlot, Coordinate, CursorStyle, Point, Screen,
    ansi::ProtectedMode,
    kitty::key::Flags,
    page::cell::SemanticContent,
    screen::{CharsetState, Options, SavedCursor},
    style::Style,
};

#[test]
fn screen_initial_defaults_and_reset() {
    for opts in [
        Options::default(),
        Options {
            cols: std::num::NonZeroU16::new(7).unwrap(),
            rows: std::num::NonZeroU16::new(3).unwrap(),
            max_scrollback: 1000,
        },
    ] {
        check_construction_and_reset(opts);
    }
}

fn check_construction_and_reset(opts: Options) {
    let mut screen = Screen::init(opts);
    assert_default_state(&screen, opts);
    let cursor_pin = screen.cursor.page_pin;
    let pin = *screen.pages.tracked_pin(cursor_pin).unwrap();
    *pin.row_and_cell_mut(&mut screen.pages).unwrap().1 = Cell::init('x');
    screen.cursor.cursor_style = CursorStyle::Bar;
    screen.cursor.pending_wrap = true;
    screen.cursor.protected = true;
    screen
        .set_attribute(maestro_terminal::sgr::Attribute::Palette256Bg(3))
        .unwrap();
    screen
        .start_hyperlink(b"https://example.test", Some(b"owned"))
        .unwrap();
    screen.cursor.hyperlink_implicit_id = 99;
    screen.cursor.semantic_content = SemanticContent::Prompt;
    screen.cursor.semantic_content_clear_eol = true;
    screen.saved_cursor = Some(SavedCursor {
        x: 4,
        y: 5,
        style: screen.cursor.style,
        protected: true,
        pending_wrap: true,
        origin: true,
        charset: CharsetState::default(),
    });
    screen.charset.gl = CharsetSlot::G1;
    screen
        .charset
        .charsets
        .set(CharsetSlot::G0, Charset::British);
    screen.charset.single_shift = Some(CharsetSlot::G3);
    screen.kitty_keyboard.push(Flags::TRUE);
    screen.protected_mode = ProtectedMode::Dec;
    screen.semantic_prompt.seen = true;
    screen.reset();
    assert_reset(&screen, cursor_pin, opts);
}

fn assert_reset(
    screen: &Screen,
    cursor_pin: maestro_terminal::page_list::TrackedPin,
    opts: Options,
) {
    assert_default_state(screen, opts);
    assert_eq!(screen.cursor.page_pin, cursor_pin);
    let pin = *screen.pages.tracked_pin(cursor_pin).unwrap();
    assert_eq!(
        screen
            .pages
            .point_from_pin(maestro_terminal::point::Tag::Active, pin),
        Some(Point::Active(Coordinate::default()))
    );
    assert!(pin.row_and_cell(&screen.pages).unwrap().1.is_zero());
    assert_eq!(screen.pages.count_tracked_pins(), 2);
}

fn assert_default_state(screen: &Screen, opts: Options) {
    assert_eq!(
        (screen.pages.cols(), screen.pages.rows()),
        (opts.cols.get(), opts.rows.get())
    );
    assert_eq!(screen.no_scrollback, opts.max_scrollback == 0);
    assert_default_cursor(&screen.cursor);
    assert_eq!(screen.charset.gl, CharsetSlot::G0);
    assert_eq!(screen.charset.gr, CharsetSlot::G2);
    assert!(screen.charset.single_shift.is_none());
    for slot in [
        CharsetSlot::G0,
        CharsetSlot::G1,
        CharsetSlot::G2,
        CharsetSlot::G3,
    ] {
        assert_eq!(screen.charset.charsets.get(slot), Charset::Utf8);
    }
    assert!(screen.saved_cursor.is_none());
    assert_eq!(screen.protected_mode, ProtectedMode::Off);
    assert!(!screen.semantic_prompt.seen);
    assert_eq!(screen.kitty_keyboard.current(), Flags::DISABLED);
}

fn assert_default_cursor(cursor: &maestro_terminal::Cursor) {
    assert_eq!((cursor.x, cursor.y), (0, 0));
    assert_eq!(cursor.cursor_style, CursorStyle::Block);
    assert_eq!(cursor.style, Style::default());
    assert_eq!(cursor.style_id, 0);
    assert!(!cursor.pending_wrap);
    assert!(!cursor.protected);
    assert_eq!(cursor.hyperlink_implicit_id, 0);
    assert_eq!(cursor.hyperlink_id, 0);
    assert!(cursor.hyperlink.is_none());
    assert_eq!(cursor.semantic_content, SemanticContent::Output);
    assert!(!cursor.semantic_content_clear_eol);
}

#[test]
fn screen_charset_slots_and_single_shift_are_independent() {
    let mut screen = Screen::init(Options::default());
    let mappings = [
        (CharsetSlot::G0, Charset::Ascii),
        (CharsetSlot::G1, Charset::British),
        (CharsetSlot::G2, Charset::DecSpecial),
        (CharsetSlot::G3, Charset::Utf8),
    ];
    for (slot, charset) in mappings {
        screen.charset.charsets.set(slot, charset);
    }
    screen.charset.gl = CharsetSlot::G1;
    assert_eq!(screen.charset.gr, CharsetSlot::G2);
    screen.charset.gr = CharsetSlot::G3;
    for shift in [Some(CharsetSlot::G0), Some(CharsetSlot::G2), None] {
        screen.charset.single_shift = shift;
        assert_eq!(screen.charset.single_shift, shift);
        assert_eq!(screen.charset.gl, CharsetSlot::G1);
        assert_eq!(screen.charset.gr, CharsetSlot::G3);
        for (slot, charset) in mappings {
            assert_eq!(screen.charset.charsets.get(slot), charset);
        }
    }
    screen.reset();
    assert_eq!(screen.charset, CharsetState::default());
}

/// Construct controlled public screen fixtures.
pub(super) fn small_screen(cols: u16, rows: u16, history: usize) -> Screen {
    Screen::init(Options {
        cols: std::num::NonZeroU16::new(cols).unwrap(),
        rows: std::num::NonZeroU16::new(rows).unwrap(),
        max_scrollback: history,
    })
}

/// Write plain fixture characters through the real cursor, with newline scrolling.
pub(super) fn write_text(screen: &mut Screen, text: &str) {
    for ch in text.chars() {
        if ch == '\n' {
            screen.cursor.pending_wrap = false;
            screen.cursor_horizontal_absolute(0);
            down_or_scroll(screen);
        } else {
            if screen.cursor.pending_wrap {
                let pin = *screen.pages.tracked_pin(screen.cursor.page_pin).unwrap();
                pin.row_and_cell_mut(&mut screen.pages)
                    .unwrap()
                    .0
                    .set_wrap(true);
                screen.cursor_horizontal_absolute(0);
                down_or_scroll(screen);
                screen.cursor.pending_wrap = false;
                let pin = *screen.pages.tracked_pin(screen.cursor.page_pin).unwrap();
                pin.row_and_cell_mut(&mut screen.pages)
                    .unwrap()
                    .0
                    .set_wrap_continuation(true);
            }
            match maestro_terminal::unicode::codepoint_width(ch) {
                1 => write_cell(screen, ch),
                2 => write_wide_cell(screen, ch),
                width => panic!("unsupported fixture width: {width}"),
            }
            if screen.cursor.x + 1 == screen.pages.cols() {
                screen.cursor.pending_wrap = true;
            } else {
                screen.cursor_right(1);
            }
        }
    }
}

/// Advance a fixture row through the screen's actual bottom-scroll owner.
fn down_or_scroll(screen: &mut Screen) {
    if screen.cursor.y + 1 == screen.pages.rows() {
        screen.cursor_down_scroll().unwrap();
    } else {
        screen.cursor_down(1);
    }
}

/// Populate an unstyled fixture cell without bypassing managed ownership.
fn write_cell(screen: &mut Screen, ch: char) {
    let pin = *screen.pages.tracked_pin(screen.cursor.page_pin).unwrap();
    screen.clear_cells(pin.node, pin.y, pin.x..pin.x + 1);
    assert_eq!(screen.cursor.style_id, 0);
    let page = screen.pages.page_mut(pin.node).unwrap();
    let cell = &mut page.get_cells_mut(pin.y)[usize::from(pin.x)];
    *cell = Cell::init(ch);
    cell.set_protected(screen.cursor.protected);
    if screen.cursor.hyperlink_id != 0 {
        screen.cursor_set_hyperlink().unwrap();
    }
}

/// Populate the source fixture's wide scalar, optional head and trailing spacer.
fn write_wide_cell(screen: &mut Screen, ch: char) {
    use maestro_terminal::page::cell::Wide;
    if screen.cursor.x + 1 == screen.pages.cols() {
        write_cell(screen, '\0');
        let pin = *screen.pages.tracked_pin(screen.cursor.page_pin).unwrap();
        let (row, cell) = pin.row_and_cell_mut(&mut screen.pages).unwrap();
        row.set_wrap(true);
        cell.set_wide(Wide::SpacerHead);
        down_or_scroll(screen);
        screen.cursor_horizontal_absolute(0);
        let pin = *screen.pages.tracked_pin(screen.cursor.page_pin).unwrap();
        pin.row_and_cell_mut(&mut screen.pages)
            .unwrap()
            .0
            .set_wrap_continuation(true);
    }
    write_cell(screen, ch);
    let pin = *screen.pages.tracked_pin(screen.cursor.page_pin).unwrap();
    pin.row_and_cell_mut(&mut screen.pages)
        .unwrap()
        .1
        .set_wide(Wide::Wide);
    screen.cursor_right(1);
    write_cell(screen, '\0');
    let pin = *screen.pages.tracked_pin(screen.cursor.page_pin).unwrap();
    pin.row_and_cell_mut(&mut screen.pages)
        .unwrap()
        .1
        .set_wide(Wide::SpacerTail);
}

/// Read real retained rows with trailing blanks removed, matching the source dump.
pub(super) fn contents(screen: &Screen, tag: maestro_terminal::point::Tag) -> String {
    let mut rows = screen.pages.row_iterator(
        maestro_terminal::page_list::Direction::RightDown,
        match tag {
            maestro_terminal::point::Tag::Active => Point::Active(Coordinate::default()),
            maestro_terminal::point::Tag::Viewport => Point::Viewport(Coordinate::default()),
            maestro_terminal::point::Tag::Screen => Point::Screen(Coordinate::default()),
            maestro_terminal::point::Tag::History => Point::History(Coordinate::default()),
        },
        None,
    );
    let mut lines = Vec::new();
    while let Some(pin) = rows.next(&screen.pages) {
        let cells = pin
            .cells(&screen.pages, maestro_terminal::page_list::CellSubset::All)
            .unwrap();
        let end = cells
            .iter()
            .rposition(|cell| cell.has_text())
            .map_or(0, |i| i + 1);
        let text: String = cells[..end]
            .iter()
            .filter(|cell| {
                !matches!(
                    cell.wide(),
                    maestro_terminal::page::cell::Wide::SpacerHead
                        | maestro_terminal::page::cell::Wide::SpacerTail
                )
            })
            .map(|cell| {
                if cell.has_text() {
                    cell.codepoint()
                } else {
                    ' '
                }
            })
            .collect();
        lines.push(text);
    }
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines.join("\n")
}

#[test]
fn screen_read_and_write() {
    let mut s = small_screen(80, 24, 1000);
    assert_eq!(s.cursor.style_id, 0);
    write_text(&mut s, "hello, world");
    assert_eq!(
        contents(&s, maestro_terminal::point::Tag::Screen),
        "hello, world"
    );
}

#[test]
fn screen_read_and_write_newline() {
    let mut s = small_screen(80, 24, 1000);
    assert_eq!(s.cursor.style_id, 0);
    write_text(&mut s, "hello\nworld");
    assert_eq!(
        contents(&s, maestro_terminal::point::Tag::Screen),
        "hello\nworld"
    );
}

#[test]
fn screen_read_and_write_scrollback() {
    let mut s = small_screen(80, 2, 1000);
    write_text(&mut s, "hello\nworld\ntest");
    assert_eq!(
        contents(&s, maestro_terminal::point::Tag::Active),
        "world\ntest"
    );
    assert_eq!(
        contents(&s, maestro_terminal::point::Tag::Screen),
        "hello\nworld\ntest"
    );
}

#[test]
fn screen_read_and_write_no_scrollback_small() {
    let mut s = small_screen(80, 2, 0);
    write_text(&mut s, "hello\nworld\ntest");
    assert_eq!(
        contents(&s, maestro_terminal::point::Tag::Active),
        "world\ntest"
    );
    assert_eq!(
        contents(&s, maestro_terminal::point::Tag::Screen),
        "world\ntest"
    );
}

#[test]
fn screen_read_and_write_no_scrollback_large() {
    let mut s = small_screen(80, 2, 0);
    for i in 0..1000 {
        write_text(&mut s, &format!("{i}\n"));
    }
    write_text(&mut s, "1000");
    assert_eq!(
        contents(&s, maestro_terminal::point::Tag::Screen),
        "999\n1000"
    );
}

#[test]
fn screen_scrolling() {
    use maestro_terminal::{color::Rgb, point::Tag, screen::Scroll, sgr::Attribute};
    let mut s = small_screen(10, 3, 0);
    write_text(&mut s, "1ABCD\n2EFGH\n3IJKL");
    s.set_attribute(Attribute::DirectColorBg(Rgb { r: 155, g: 0, b: 0 }))
        .unwrap();
    s.cursor_down_scroll().unwrap();
    assert_eq!(contents(&s, Tag::Viewport), "2EFGH\n3IJKL");
    let bottom = s
        .pages
        .pin(Point::Active(Coordinate { x: 0, y: 2 }))
        .unwrap();
    assert_eq!(*bottom.row_and_cell(&s.pages).unwrap().1, s.blank_cell());
    for y in 0..3 {
        let pin = s.pages.pin(Point::Active(Coordinate { x: 0, y })).unwrap();
        assert!(pin.row_and_cell(&s.pages).unwrap().0.dirty());
    }
    s.scroll(Scroll::Active);
    assert_eq!(contents(&s, Tag::Viewport), "2EFGH\n3IJKL");
}

#[test]
fn screen_scrolling_with_a_single_row_screen_no_scrollback() {
    let mut s = small_screen(10, 1, 0);
    write_text(&mut s, "1ABCD");
    s.cursor_down_scroll().unwrap();
    assert_eq!(contents(&s, maestro_terminal::point::Tag::Viewport), "");
    let pin = *s.pages.tracked_pin(s.cursor.page_pin).unwrap();
    assert!(pin.row_and_cell(&s.pages).unwrap().0.dirty());
}

#[test]
fn screen_scrolling_with_a_single_row_screen_with_scrollback() {
    use maestro_terminal::{point::Tag, screen::Scroll};
    let mut s = small_screen(10, 1, 1);
    write_text(&mut s, "1ABCD");
    s.cursor_down_scroll().unwrap();
    assert_eq!(contents(&s, Tag::Viewport), "");
    for point in [
        Point::Active(Coordinate::default()),
        Point::Screen(Coordinate::default()),
    ] {
        let pin = s.pages.pin(point).unwrap();
        assert!(pin.row_and_cell(&s.pages).unwrap().0.dirty());
    }
    s.scroll(Scroll::DeltaRow(-1));
    assert_eq!(contents(&s, Tag::Viewport), "1ABCD");
}

#[test]
fn screen_scroll_down_from_0() {
    let mut s = small_screen(10, 3, 0);
    write_text(&mut s, "1ABCD\n2EFGH\n3IJKL");
    s.scroll(maestro_terminal::screen::Scroll::DeltaRow(-1));
    assert!(s.viewport_is_bottom());
    assert_eq!(
        contents(&s, maestro_terminal::point::Tag::Viewport),
        "1ABCD\n2EFGH\n3IJKL"
    );
}

#[test]
fn screen_scrollback_various_cases() {
    use maestro_terminal::{point::Tag, screen::Scroll};
    let mut s = small_screen(10, 3, 1);
    write_text(&mut s, "1ABCD\n2EFGH\n3IJKL");
    s.cursor_down_scroll().unwrap();
    assert_eq!(contents(&s, Tag::Viewport), "2EFGH\n3IJKL");
    s.scroll(Scroll::Active);
    assert_eq!(contents(&s, Tag::Viewport), "2EFGH\n3IJKL");
    for _ in 0..2 {
        s.scroll(Scroll::DeltaRow(-1));
        assert!(!s.viewport_is_bottom());
        assert_eq!(contents(&s, Tag::Viewport), "1ABCD\n2EFGH\n3IJKL");
    }
    for behavior in [Scroll::Active, Scroll::DeltaRow(1)] {
        s.scroll(behavior);
        assert_eq!(contents(&s, Tag::Viewport), "2EFGH\n3IJKL");
    }
    s.scroll(Scroll::Top);
    assert_eq!(contents(&s, Tag::Viewport), "1ABCD\n2EFGH\n3IJKL");
    s.clear_rows(Point::Active(Coordinate::default()), None, false);
    assert_eq!(contents(&s, Tag::Viewport), "1ABCD");
    s.scroll(Scroll::Active);
    assert_eq!(contents(&s, Tag::Viewport), "");
}

#[test]
fn screen_scrollback_with_multi_row_delta() {
    use maestro_terminal::{point::Tag, screen::Scroll};
    let mut s = small_screen(10, 3, 3);
    write_text(&mut s, "1ABCD\n2EFGH\n3IJKL\n4ABCD\n5EFGH\n6IJKL");
    s.scroll(Scroll::Top);
    assert_eq!(contents(&s, Tag::Viewport), "1ABCD\n2EFGH\n3IJKL");
    s.scroll(Scroll::DeltaRow(5));
    assert!(s.viewport_is_bottom());
    assert_eq!(contents(&s, Tag::Viewport), "4ABCD\n5EFGH\n6IJKL");
}

#[test]
fn screen_scrollback_empty() {
    let mut s = small_screen(10, 3, 50);
    write_text(&mut s, "1ABCD\n2EFGH\n3IJKL");
    s.scroll(maestro_terminal::screen::Scroll::DeltaRow(1));
    assert_eq!(
        contents(&s, maestro_terminal::point::Tag::Viewport),
        "1ABCD\n2EFGH\n3IJKL"
    );
}

#[test]
fn screen_scrollback_doesn_t_move_viewport_if_not_at_bottom() {
    let mut s = small_screen(10, 3, 3);
    write_text(&mut s, "1ABCD\n2EFGH\n3IJKL\n4ABCD\n5EFGH");
    s.scroll(maestro_terminal::screen::Scroll::DeltaRow(-1));
    assert_eq!(
        contents(&s, maestro_terminal::point::Tag::Viewport),
        "2EFGH\n3IJKL\n4ABCD"
    );
    for _ in 0..2 {
        s.cursor_down_scroll().unwrap();
        assert_eq!(
            contents(&s, maestro_terminal::point::Tag::Viewport),
            "2EFGH\n3IJKL\n4ABCD"
        );
    }
}

#[test]
fn screen_scrolling_moves_viewport() {
    use maestro_terminal::{point::Tag, screen::Scroll};
    let mut s = small_screen(10, 3, 1);
    write_text(&mut s, "1ABCD\n2EFGH\n3IJKL\n1ABCD\n2EFGH\n3IJKL");
    s.scroll(Scroll::DeltaRow(-2));
    assert_eq!(contents(&s, Tag::Viewport), "2EFGH\n3IJKL\n1ABCD");
    assert_eq!(
        s.pages
            .point_from_pin(Tag::Screen, s.pages.get_top_left(Tag::Viewport)),
        Some(Point::Screen(Coordinate { x: 0, y: 1 }))
    );
}

#[test]
fn screen_scrolling_when_viewport_is_pruned() {
    use maestro_terminal::{point::Tag, screen::Scroll};
    let mut s = small_screen(215, 3, 1);
    write_text(&mut s, "1ABCD\n2EFGH\n3IJKL\n1ABCD\n2EFGH\n3IJKL");
    s.scroll(Scroll::DeltaRow(-2));
    write_text(&mut s, "\n");
    for _ in 0..1000 {
        write_text(&mut s, "1ABCD\n2EFGH\n3IJKL\n");
    }
    write_text(&mut s, "1ABCD\n2EFGH\n3IJKL");
    assert_eq!(
        s.pages
            .point_from_pin(Tag::Screen, s.pages.get_top_left(Tag::Viewport)),
        Some(Point::Screen(Coordinate::default()))
    );
}

#[test]
fn screen_scroll_and_clear_full_screen() {
    check_scroll_clear("1ABCD\n2EFGH\n3IJKL");
}

#[test]
fn screen_scroll_and_clear_partial_screen() {
    check_scroll_clear("1ABCD\n2EFGH");
}

#[test]
fn screen_scroll_and_clear_empty_screen() {
    check_scroll_clear("");
}

/// Check both retained and active observations around scroll clearing.
fn check_scroll_clear(text: &str) {
    use maestro_terminal::point::Tag;
    let mut s = small_screen(10, 3, 5);
    write_text(&mut s, text);
    assert_eq!(contents(&s, Tag::Viewport), text);
    s.scroll_clear();
    assert_eq!(contents(&s, Tag::Viewport), "");
    assert_eq!(contents(&s, Tag::Screen), text);
}

#[test]
fn screen_scroll_and_clear_ignore_blank_lines() {
    use maestro_terminal::point::Tag;
    let mut s = small_screen(10, 3, 10);
    write_text(&mut s, "1ABCD\n2EFGH");
    s.scroll_clear();
    assert_eq!(contents(&s, Tag::Viewport), "");
    s.cursor_absolute(0, 0);
    write_text(&mut s, "3ABCD\n");
    assert_eq!(contents(&s, Tag::Active), "3ABCD");
    s.scroll_clear();
    assert_eq!(contents(&s, Tag::Viewport), "");
    s.cursor_absolute(0, 0);
    write_text(&mut s, "X");
    assert_eq!(contents(&s, Tag::Screen), "1ABCD\n2EFGH\n3ABCD\nX");
}

#[test]
fn screen_erase_rows_history() {
    check_erase_history("1\n2\n3\n4\n5\n6");
}

#[test]
fn screen_erase_rows_history_with_more_lines() {
    check_erase_history("A\nB\nC\n1\n2\n3\n4\n5\n6");
}

/// Keep the same active rows while physically removing retained history.
fn check_erase_history(text: &str) {
    use maestro_terminal::point::Tag;
    let mut s = small_screen(5, 5, 1000);
    write_text(&mut s, text);
    assert_eq!(contents(&s, Tag::Active), "2\n3\n4\n5\n6");
    assert_eq!(contents(&s, Tag::Screen), text);
    s.erase_history(None);
    assert_eq!(contents(&s, Tag::Active), "2\n3\n4\n5\n6");
    assert_eq!(contents(&s, Tag::Screen), "2\n3\n4\n5\n6");
}

#[test]
fn screen_erase_rows_active_partial() {
    use maestro_terminal::point::Tag;
    let mut s = small_screen(5, 5, 0);
    write_text(&mut s, "1\n2\n3");
    assert_eq!(contents(&s, Tag::Active), "1\n2\n3");
    s.erase_active(1);
    assert_eq!(contents(&s, Tag::Active), "3");
    assert_eq!(contents(&s, Tag::Screen), "3");
}

#[test]
fn screen_clear_history_with_no_history() {
    use maestro_terminal::point::Tag;
    let mut s = small_screen(10, 3, 3);
    write_text(&mut s, "4ABCD\n5EFGH\n6IJKL");
    assert!(s.viewport_is_bottom());
    s.erase_history(None);
    assert!(s.viewport_is_bottom());
    assert_eq!(contents(&s, Tag::Viewport), "4ABCD\n5EFGH\n6IJKL");
    assert_eq!(contents(&s, Tag::Screen), "4ABCD\n5EFGH\n6IJKL");
}

#[test]
fn screen_clear_history() {
    use maestro_terminal::{point::Tag, screen::Scroll};
    let mut s = small_screen(10, 3, 3);
    write_text(&mut s, "1ABCD\n2EFGH\n3IJKL\n4ABCD\n5EFGH\n6IJKL");
    assert!(s.viewport_is_bottom());
    s.scroll(Scroll::Top);
    assert_eq!(contents(&s, Tag::Viewport), "1ABCD\n2EFGH\n3IJKL");
    s.erase_history(None);
    assert!(s.viewport_is_bottom());
    assert_eq!(contents(&s, Tag::Viewport), "4ABCD\n5EFGH\n6IJKL");
    assert_eq!(contents(&s, Tag::Screen), "4ABCD\n5EFGH\n6IJKL");
}

#[test]
fn screen_clear_above_cursor() {
    check_clear_above_cursor(10, "", "\n\n6IJKL");
}

#[test]
fn screen_clear_above_cursor_with_history() {
    check_clear_above_cursor(3, "1ABCD\n2EFGH\n3IJKL\n", "1ABCD\n2EFGH\n3IJKL\n\n\n6IJKL");
}

/// Clearing active rows must not touch history or cursor coordinates.
fn check_clear_above_cursor(rows: u16, prefix: &str, expected: &str) {
    use maestro_terminal::point::Tag;
    let mut s = small_screen(10, rows, 3);
    write_text(&mut s, prefix);
    write_text(&mut s, "4ABCD\n5EFGH\n6IJKL");
    s.clear_rows(
        Point::Active(Coordinate::default()),
        Some(Point::Active(Coordinate {
            x: 0,
            y: u32::from(s.cursor.y - 1),
        })),
        false,
    );
    assert_eq!(contents(&s, Tag::Viewport), "\n\n6IJKL");
    assert_eq!(contents(&s, Tag::Screen), expected);
    assert_eq!((s.cursor.x, s.cursor.y), (5, 2));
}

#[test]
fn screen_cursor_scroll_region_up_simple() {
    let mut s = small_screen(5, 5, 0);
    write_text(&mut s, "1ABCD\n2EFGH\n3IJKL\n4MNOP\n5QRST");
    s.cursor_absolute(1, 2);
    s.cursor_scroll_region_up(2).unwrap();
    assert_eq!((s.cursor.x, s.cursor.y), (1, 2));
    assert_eq!(
        contents(&s, maestro_terminal::point::Tag::Screen),
        "2EFGH\n3IJKL\n\n4MNOP\n5QRST"
    );
}

#[test]
fn screen_region_scroll_preserves_live_page_identity() {
    let mut s = small_screen(5, 5, 0);
    write_text(&mut s, "1ABCD\n2EFGH\n3IJKL\n4MNOP\n5QRST");
    s.cursor_absolute(0, 2);
    let old = *s.pages.tracked_pin(s.cursor.page_pin).unwrap();
    s.cursor_scroll_region_up(2).unwrap();
    assert!(s.pages.page(old.node).is_some());
    assert_eq!(*s.pages.tracked_pin(s.cursor.page_pin).unwrap(), old);
    assert_eq!(
        contents(&s, maestro_terminal::point::Tag::Screen),
        "2EFGH\n3IJKL\n\n4MNOP\n5QRST"
    );
}

/// Place the active origin near the real first-page boundary.
fn boundary_screen(rows: u16, first_active: u16) -> Screen {
    let mut s = small_screen(10, rows, 100_000);
    let pin = *s.pages.tracked_pin(s.cursor.page_pin).unwrap();
    let capacity = s.pages.page(pin.node).unwrap().capacity.rows;
    for _ in 0..capacity - first_active {
        write_text(&mut s, "\n");
    }
    s
}

#[test]
fn screen_cursor_scroll_region_up_region_spans_pages() {
    let mut s = boundary_screen(5, 3);
    write_text(&mut s, "1A\n2B\n3C\n4D\n5E");
    s.cursor_absolute(0, 3);
    s.set_attribute(maestro_terminal::sgr::Attribute::Bold)
        .unwrap();
    let old = *s.pages.tracked_pin(s.cursor.page_pin).unwrap();
    assert_eq!(old.y, 0);
    s.cursor_scroll_region_up(2).unwrap();
    assert_eq!((s.cursor.x, s.cursor.y), (0, 3));
    assert_eq!(*s.pages.tracked_pin(s.cursor.page_pin).unwrap(), old);
    assert_eq!(
        contents(&s, maestro_terminal::point::Tag::Viewport),
        "1A\n3C\n4D\n\n5E"
    );
    s.set_attribute(maestro_terminal::sgr::Attribute::Unset)
        .unwrap();
}

#[test]
fn screen_cursor_scroll_region_up_region_spans_pages_with_background_sgr() {
    let mut s = boundary_screen(5, 3);
    write_text(&mut s, "1A\n2B\n3C\n4D\n5E");
    s.cursor_absolute(0, 3);
    s.set_attribute(maestro_terminal::sgr::Attribute::DirectColorBg(
        maestro_terminal::color::Rgb { r: 255, g: 0, b: 0 },
    ))
    .unwrap();
    s.cursor_scroll_region_up(2).unwrap();
    assert_eq!(
        contents(&s, maestro_terminal::point::Tag::Viewport),
        "1A\n3C\n4D\n\n5E"
    );
    for x in 0..10 {
        assert_eq!(
            s.pages
                .get_cell(Point::Active(Coordinate { x, y: 3 }))
                .unwrap()
                .cell
                .content(),
            maestro_terminal::page::cell::Content::BgColorRgb(maestro_terminal::color::Rgb {
                r: 255,
                g: 0,
                b: 0
            })
        );
    }
}

/// Observe insertion, background fill, dirty rows and managed cursor migration.
fn check_scroll_above(mut s: Screen, text: &str, expected: &str) {
    write_text(&mut s, text);
    s.cursor_absolute(0, 1);
    s.set_attribute(maestro_terminal::sgr::Attribute::DirectColorBg(
        maestro_terminal::color::Rgb { r: 155, g: 0, b: 0 },
    ))
    .unwrap();
    let mut rows = s.pages.row_iterator(
        maestro_terminal::page_list::Direction::RightDown,
        Point::Screen(Coordinate::default()),
        None,
    );
    while let Some(pin) = rows.next(&s.pages) {
        s.pages.page_mut(pin.node).unwrap().dirty = false;
        pin.row_and_cell_mut(&mut s.pages)
            .unwrap()
            .0
            .set_dirty(false);
    }
    let old = *s.pages.tracked_pin(s.cursor.page_pin).unwrap();
    s.cursor_scroll_above().unwrap();
    assert!(s.pages.page(old.node).is_some());
    assert_eq!(
        contents(&s, maestro_terminal::point::Tag::Viewport),
        expected
    );
    assert_eq!((s.cursor.x, s.cursor.y), (0, 1));
    for y in 0..u32::from(s.pages.rows()) {
        assert_eq!(
            s.pages
                .pin(Point::Active(Coordinate { x: 0, y }))
                .unwrap()
                .is_dirty(&s.pages),
            Some(true)
        );
    }
    let current = *s.pages.tracked_pin(s.cursor.page_pin).unwrap();
    for cell in s.pages.page(current.node).unwrap().get_cells(current.y) {
        assert_eq!(
            cell.content(),
            maestro_terminal::page::cell::Content::BgColorRgb(maestro_terminal::color::Rgb {
                r: 155,
                g: 0,
                b: 0
            })
        );
    }
    s.set_attribute(maestro_terminal::sgr::Attribute::ResetBg)
        .unwrap();
}

#[test]
fn screen_scroll_above_same_page() {
    check_scroll_above(
        small_screen(10, 3, 100_000),
        "1ABCD\n2EFGH\n3IJKL",
        "2EFGH\n\n3IJKL",
    );
}

#[test]
fn screen_scroll_above_same_page_but_cursor_on_previous_page() {
    check_scroll_above(
        boundary_screen(5, 3),
        "1A\n2B\n3C\n4D\n5E",
        "2B\n\n3C\n4D\n5E",
    );
}

#[test]
fn screen_scroll_above_same_page_but_cursor_on_previous_page_last_row() {
    check_scroll_above(
        boundary_screen(5, 2),
        "1A\n2B\n3C\n4D\n5E",
        "2B\n\n3C\n4D\n5E",
    );
}

#[test]
fn screen_scroll_above_creates_new_page() {
    check_scroll_above(
        boundary_screen(3, 3),
        "1ABCD\n2EFGH\n3IJKL",
        "2EFGH\n\n3IJKL",
    );
}

#[test]
fn screen_scroll_above_with_cursor_on_non_final_row() {
    check_scroll_above(
        boundary_screen(4, 3),
        "1AB\n2BC\n3DE\n4FG",
        "2BC\n\n3DE\n4FG",
    );
}

#[test]
fn screen_scroll_above_no_scrollback_bottom_of_page() {
    let mut s = small_screen(10, 3, 0);
    for _ in 0..100 {
        write_text(&mut s, "\n");
    }
    check_scroll_above(s, "1ABCD\n2EFGH\n3IJKL", "2EFGH\n\n3IJKL");
}

#[test]
fn screen_saved_cursor_follows_resize_and_loses_invalid_wrap() {
    let mut s = small_screen(5, 3, 100_000);
    write_text(&mut s, "ABCDE");
    s.charset.gl = maestro_terminal::CharsetSlot::G3;
    s.charset.charsets.set(
        maestro_terminal::CharsetSlot::G3,
        maestro_terminal::Charset::Ascii,
    );
    let style = Style {
        bg_color: maestro_terminal::style::Color::Palette(7),
        ..Style::default()
    };
    s.saved_cursor = Some(SavedCursor {
        x: 4,
        y: 0,
        style,
        protected: true,
        pending_wrap: true,
        origin: true,
        charset: s.charset,
    });
    s.resize(maestro_terminal::screen::Resize {
        cols: std::num::NonZeroU16::new(10).unwrap(),
        rows: std::num::NonZeroU16::new(3).unwrap(),
        reflow: true,
        prompt_redraw: maestro_terminal::osc::semantic_prompt::Redraw::False,
    })
    .unwrap();
    let saved = s.saved_cursor.unwrap();
    assert_eq!((saved.x, saved.y, saved.pending_wrap), (5, 0, false));
    assert!(saved.protected && saved.origin);
    assert_eq!(saved.style, style);
    assert_eq!(saved.charset, s.charset);
    s.resize(maestro_terminal::screen::Resize {
        cols: std::num::NonZeroU16::new(3).unwrap(),
        rows: std::num::NonZeroU16::new(2).unwrap(),
        reflow: true,
        prompt_redraw: maestro_terminal::osc::semantic_prompt::Redraw::False,
    })
    .unwrap();
    let shrunk = s.saved_cursor.unwrap();
    assert_eq!((shrunk.x, shrunk.y, shrunk.pending_wrap), (2, 0, false));
    assert_eq!(shrunk.style, style);
    assert_eq!(shrunk.charset, saved.charset);
    assert!(shrunk.protected && shrunk.origin);
    assert_eq!(s.pages.count_tracked_pins(), 2);
}
