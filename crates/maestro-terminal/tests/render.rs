//! Retained viewport contents and invalidation through borrowed rows and cells.

use maestro_terminal::{
    Terminal,
    render::{Dirty, RenderState},
    terminal::ScrollViewport,
};

fn terminal(text: &str) -> Terminal {
    let mut t = Terminal::new(10, 3, 100_000).unwrap();
    feed(&mut t, text);
    t
}

fn feed(t: &mut Terminal, text: &str) {
    t.write(text.as_bytes());
}

fn clean(state: &mut RenderState) {
    for mut row in state.rows() {
        row.clear_dirty();
    }
    state.dirty = Dirty::Clean;
}

#[test]
fn basic_text() {
    let mut t = terminal("ABCD");
    let mut state = RenderState::default();
    state.update(&mut t);
    assert_eq!((state.cols, state.rows), (10, 3));
    let rows: Vec<_> = state.rows().collect();
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|row| row.cells().count() == 10));
    assert_eq!(
        rows[0]
            .cells()
            .map(|c| c.graphemes_utf8())
            .collect::<String>(),
        "ABCD"
    );
}

#[test]
fn dirty_state() {
    let mut t = terminal("");
    let mut state = RenderState::default();
    state.update(&mut t);
    assert_eq!(state.dirty, Dirty::Full);
    assert!(state.rows().all(|row| row.dirty()));
    clean(&mut state);
    state.update(&mut t);
    assert_eq!(state.dirty, Dirty::Clean);
    assert!(state.rows().all(|row| !row.dirty()));
    feed(&mut t, "A");
    state.update(&mut t);
    assert_eq!(state.dirty, Dirty::Partial);
    assert_eq!(
        state.rows().map(|r| r.dirty()).collect::<Vec<_>>(),
        [true, false, false]
    );
}

#[test]
fn clearing_one_dirty_row_preserves_other_rows() {
    let mut t = terminal("A\r\nB");
    let mut state = RenderState::default();
    state.update(&mut t);
    state.rows().next().unwrap().clear_dirty();
    assert_eq!(
        state.rows().map(|r| r.dirty()).collect::<Vec<_>>(),
        [false, true, true]
    );
    assert_eq!(state.dirty, Dirty::Full);
}

fn reverse_video_transition(enabled: bool) {
    let mut t = terminal("hello");
    let bg = maestro_terminal::color::Rgb { r: 1, g: 2, b: 3 };
    let fg = maestro_terminal::color::Rgb { r: 4, g: 5, b: 6 };
    t.set_default_background_color(Some(bg));
    t.set_default_foreground_color(Some(fg));
    let mut state = RenderState::default();
    state.update(&mut t);
    if !enabled {
        feed(&mut t, "\x1b[?5h");
        state.update(&mut t);
    }
    let (bytes, background, foreground) = if enabled {
        ("\x1b[?5h", fg, bg)
    } else {
        ("\x1b[?5l", bg, fg)
    };
    {
        clean(&mut state);
        feed(&mut t, bytes);
        state.update(&mut t);
        assert_eq!(state.dirty, Dirty::Full);
        assert!(state.rows().all(|r| r.dirty()));
        assert_eq!(
            (state.colors.background, state.colors.foreground),
            (background, foreground)
        );
        clean(&mut state);
        feed(&mut t, bytes);
        state.update(&mut t);
        assert_eq!(state.dirty, Dirty::Clean);
    }
}

#[test]
fn reverse_video_set_invalidates_the_screen() {
    reverse_video_transition(true);
}

#[test]
fn reverse_video_reset_invalidates_the_screen() {
    reverse_video_transition(false);
}

#[test]
fn snapshot_cells_resolve_packed_styles() {
    let mut t = terminal("\x1b[41m\x1b#8");
    let mut state = RenderState::default();
    state.update(&mut t);
    assert!(
        state
            .rows()
            .all(|row| row.cells().all(|c| c.raw().codepoint() == 'E'
                && c.background() == Some(maestro_terminal::color::DEFAULT[1])))
    );
}

#[test]
fn styled_text() {
    let mut t = terminal("\x1b[1mA\x1b[0;3mB\x1b[0;4mC");
    let mut state = RenderState::default();
    state.update(&mut t);
    let row = state.rows().next().unwrap();
    assert!(row.cell(0).unwrap().style().flags.bold());
    assert!(row.cell(1).unwrap().style().flags.italic());
    assert!(!row.cell(1).unwrap().style().flags.bold());
    assert_eq!(
        row.cell(2).unwrap().style().flags.underline(),
        maestro_terminal::sgr::Underline::Single
    );
    assert!(row.cell(3).unwrap().style().is_default());
    assert!(row.cell(10).is_none());
}

#[test]
fn bg_color_cells() {
    let mut t = terminal("\x1b[1mA\x1b[48;5;1m\x1b[K");
    let mut state = RenderState::default();
    state.update(&mut t);
    let row = state.rows().next().unwrap();
    assert!(row.cell(0).unwrap().style().flags.bold());
    for cell in row.cells().skip(1) {
        assert_eq!(
            cell.raw().content(),
            maestro_terminal::page::cell::Content::BgColorPalette(1)
        );
        assert_eq!(
            cell.style().bg_color,
            maestro_terminal::style::Color::Palette(1)
        );
        assert_eq!(cell.background(), Some(maestro_terminal::color::DEFAULT[1]));
    }
}

#[test]
fn snapshot_cells_preserve_unicode_clusters() {
    let mut t = terminal("A👨\u{200d}");
    let mut state = RenderState::default();
    state.update(&mut t);
    let row = state.rows().next().unwrap();
    let cell = row.cell(1).unwrap();
    assert_eq!(cell.raw().codepoint(), '👨');
    assert_eq!(cell.raw().grid_width(), 2);
    assert_eq!(cell.grapheme(), ['\u{200d}']);
    assert_eq!(row.cell(2).unwrap().graphemes_utf8(), "");
}

#[test]
fn row_cells_get_graphemes_utf8() {
    for (input, expected) in [
        ("e\u{301}", "e\u{301}"),
        ("\x1b[?2027h🇺🇸", "🇺🇸"),
        (
            "\x1b[?2027h👨\u{200d}👩\u{200d}👧",
            "👨\u{200d}👩\u{200d}👧",
        ),
    ] {
        let mut t = terminal(input);
        let mut state = RenderState::default();
        state.update(&mut t);
        let row = state.rows().next().unwrap();
        assert_eq!(row.cell(0).unwrap().graphemes_utf8(), expected);
        assert_eq!(row.cell(1).unwrap().graphemes_utf8(), "");
    }
}

#[test]
fn row_cells_get_has_styling() {
    let mut t = terminal("A\x1b[31mB");
    let mut state = RenderState::default();
    state.update(&mut t);
    let row = state.rows().next().unwrap();
    assert!(!row.cell(0).unwrap().has_styling());
    assert!(row.cell(1).unwrap().has_styling());
}

#[test]
fn row_iterator_next() {
    let mut t = terminal("A\r\nB\r\nC");
    let mut state = RenderState::default();
    state.update(&mut t);
    let mut rows = state.rows();
    for (i, expected) in ['A', 'B', 'C'].into_iter().enumerate() {
        assert_eq!(rows.len(), 3 - i);
        let row = rows.next().unwrap();
        assert_eq!(row.cell(0).unwrap().raw().codepoint(), expected);
    }
    assert!(rows.next().is_none());
}

#[test]
fn update() {
    let mut t = terminal("Hello");
    let mut state = RenderState::default();
    state.update(&mut t);
    assert_eq!((state.cols, state.rows), (10, 3));
    assert_eq!(state.cursor.active.x, 5);
    feed(&mut t, "!");
    state.update(&mut t);
    assert_eq!(
        state
            .rows()
            .next()
            .unwrap()
            .cell(5)
            .unwrap()
            .graphemes_utf8(),
        "!"
    );
}

#[test]
fn snapshot_resolves_terminal_default_colors() {
    let mut t = terminal("");
    let mut state = RenderState::default();
    state.update(&mut t);
    let empty = maestro_terminal::render::Colors::default();
    assert_eq!(state.colors, empty);
    feed(&mut t, "\x1b]12;#FF0000\x07");
    state.update(&mut t);
    assert_eq!(
        state.colors.cursor,
        Some(maestro_terminal::color::Rgb { r: 255, g: 0, b: 0 })
    );
    feed(&mut t, "\x1b]4;0;#FFFFFF\x07");
    clean(&mut state);
    state.update(&mut t);
    assert_eq!(
        state.colors.palette[0],
        maestro_terminal::color::Rgb {
            r: 255,
            g: 255,
            b: 255
        }
    );
    assert_eq!(state.dirty, Dirty::Full);
}

#[test]
fn colors_get() {
    let mut t = terminal("\x1b]10;#112233\x07\x1b]11;#445566\x07\x1b]12;#778899\x07");
    let mut state = RenderState::default();
    state.update(&mut t);
    assert_eq!(
        state.colors.foreground,
        maestro_terminal::color::Rgb {
            r: 0x11,
            g: 0x22,
            b: 0x33
        }
    );
    assert_eq!(
        state.colors.background,
        maestro_terminal::color::Rgb {
            r: 0x44,
            g: 0x55,
            b: 0x66
        }
    );
    assert_eq!(
        state.colors.cursor,
        Some(maestro_terminal::color::Rgb {
            r: 0x77,
            g: 0x88,
            b: 0x99
        })
    );
}

#[test]
fn row_cells_bg_color_no_background() {
    let mut t = terminal("A");
    let mut state = RenderState::default();
    state.update(&mut t);
    assert_eq!(
        state.rows().next().unwrap().cell(0).unwrap().background(),
        None
    );
}

#[test]
fn row_cells_bg_color_from_style() {
    let mut t = terminal("\x1b[48;2;17;34;51mA");
    let mut state = RenderState::default();
    state.update(&mut t);
    assert_eq!(
        state.rows().next().unwrap().cell(0).unwrap().background(),
        Some(maestro_terminal::color::Rgb {
            r: 17,
            g: 34,
            b: 51
        })
    );
}

#[test]
fn row_cells_bg_color_from_content_tag() {
    for (input, expected) in [
        (
            "\x1b[48;2;17;34;51m\x1b[K",
            maestro_terminal::color::Rgb {
                r: 17,
                g: 34,
                b: 51,
            },
        ),
        ("\x1b[41m\x1b[K", maestro_terminal::color::DEFAULT[1]),
    ] {
        let mut t = terminal(input);
        let mut state = RenderState::default();
        state.update(&mut t);
        let row = state.rows().next().unwrap();
        let cell = row.cell(0).unwrap();
        assert!(!cell.has_styling());
        assert_eq!(cell.background(), Some(expected));
        assert_eq!(cell.graphemes_utf8(), "");
    }
}

#[test]
fn row_cells_fg_color_no_foreground() {
    let mut t = terminal("A");
    let mut state = RenderState::default();
    state.update(&mut t);
    assert_eq!(
        state.rows().next().unwrap().cell(0).unwrap().foreground(),
        None
    );
}

#[test]
fn row_cells_fg_color_from_style() {
    let mut t = terminal("\x1b[38;2;17;34;51mA\x1b[31mB");
    let mut state = RenderState::default();
    state.update(&mut t);
    let row = state.rows().next().unwrap();
    assert_eq!(
        row.cell(0).unwrap().foreground(),
        Some(maestro_terminal::color::Rgb {
            r: 17,
            g: 34,
            b: 51
        })
    );
    assert_eq!(
        row.cell(1).unwrap().foreground(),
        Some(maestro_terminal::color::DEFAULT[1])
    );
}

#[test]
fn foreground_bold_and_underline_use_shared_style_resolution() {
    use maestro_terminal::{
        color::{DEFAULT, Rgb},
        style::BoldColor,
    };
    let mut t = terminal("\x1b[1;31;58;5;5mA\x1b[39mB");
    let mut state = RenderState::default();
    state.update(&mut t);
    let row = state.rows().next().unwrap();
    let cell = row.cell(0).unwrap();
    assert_eq!(
        cell.foreground_with(Rgb::default(), Some(BoldColor::Bright)),
        DEFAULT[9]
    );
    assert_eq!(cell.underline_color(), Some(DEFAULT[5]));
    let custom = Rgb { r: 9, g: 8, b: 7 };
    assert_eq!(
        row.cell(1)
            .unwrap()
            .foreground_with(Rgb::default(), Some(BoldColor::Color(custom))),
        custom
    );
    assert_eq!(row.cell(1).unwrap().foreground_with(custom, None), custom);
    assert_eq!(row.cell(2).unwrap().underline_color(), None);
}

#[test]
fn cursor_state_in_viewport() {
    let mut t = terminal("A\x1b[H");
    let mut state = RenderState::default();
    state.update(&mut t);
    assert_eq!(
        state.cursor.active,
        maestro_terminal::Coordinate { x: 0, y: 0 }
    );
    assert_eq!(state.cursor.cell.codepoint(), 'A');
    assert!(state.cursor.style.is_default());
    clean(&mut state);
    feed(&mut t, "\x1b[1m\x1b[2;3H\x1b[?12h\x1b[5 q");
    state.update(&mut t);
    assert_eq!(
        state.cursor.active,
        maestro_terminal::Coordinate { x: 2, y: 1 }
    );
    assert_eq!(state.cursor.viewport.unwrap().x, 2);
    assert_eq!(state.cursor.viewport.unwrap().y, 1);
    assert!(state.cursor.style.flags.bold());
    assert!(state.cursor.blinking);
    assert_eq!(
        state.cursor.visual_style,
        maestro_terminal::cursor::Style::Bar
    );
}

#[test]
fn cursor_state_out_of_viewport() {
    let mut t = terminal("A\r\nB\r\nC\r\nD\r\nE\r\n");
    let mut state = RenderState::default();
    state.update(&mut t);
    assert_eq!(state.cursor.viewport.unwrap().y, 2);
    clean(&mut state);
    t.scroll_viewport(maestro_terminal::terminal::ScrollViewport::Top);
    state.update(&mut t);
    assert_eq!(state.cursor.active.y, 2);
    assert_eq!(state.cursor.viewport, None);
    assert_eq!(state.dirty, Dirty::Full);
}

#[test]
fn alternate_screen_and_cursor_visibility_follow_modes() {
    let mut t = terminal("A");
    let mut state = RenderState::default();
    state.update(&mut t);
    assert!(state.cursor.visible);
    clean(&mut state);
    feed(&mut t, "\x1b[?1049h\x1b[H\x1b[?25lB");
    state.update(&mut t);
    assert_eq!(state.dirty, Dirty::Full);
    assert!(!state.cursor.visible);
    assert_eq!(
        state
            .rows()
            .next()
            .unwrap()
            .cell(0)
            .unwrap()
            .graphemes_utf8(),
        "B"
    );
    feed(&mut t, "\x1b[?1049l\x1b[?25h");
    state.update(&mut t);
    assert!(state.cursor.visible);
    assert_eq!(
        state
            .rows()
            .next()
            .unwrap()
            .cell(0)
            .unwrap()
            .graphemes_utf8(),
        "A"
    );
}

#[test]
fn cursor_on_wide_tail() {
    let mut t = terminal("字\x1b[2G");
    let mut state = RenderState::default();
    state.update(&mut t);
    assert!(state.cursor.viewport.unwrap().wide_tail);
    feed(&mut t, "\x1b[1G");
    state.update(&mut t);
    assert!(!state.cursor.viewport.unwrap().wide_tail);
}

#[test]
fn retained_cells_outlive_terminal_edits_and_drop() {
    let mut t = terminal("\x1b[31me\u{301}");
    let mut state = RenderState::default();
    state.update(&mut t);
    feed(&mut t, "\x1b[2J");
    drop(t);
    let row = state.rows().next().unwrap();
    assert_eq!(row.cell(0).unwrap().graphemes_utf8(), "e\u{301}");
    assert_eq!(
        row.cell(0).unwrap().foreground(),
        Some(maestro_terminal::color::DEFAULT[1])
    );
}

#[test]
fn resize_rebuilds_all_rows_and_columns() {
    let mut t = terminal("abc");
    let mut state = RenderState::default();
    for (cols, rows) in [(10, 3), (4, 5), (15, 2)] {
        t.resize(cols, rows, 8, 16).unwrap();
        state.update(&mut t);
        assert_eq!(state.dirty, Dirty::Full);
        assert_eq!(state.rows().len(), usize::from(rows));
        assert!(
            state
                .rows()
                .all(|row| row.cells().count() == usize::from(cols))
        );
        clean(&mut state);
    }
}

#[test]
fn dirty_flags_persist_until_acknowledged() {
    let mut t = terminal("A");
    let mut state = RenderState::default();
    state.update(&mut t);
    state.update(&mut t);
    assert_eq!(state.dirty, Dirty::Full);
    assert!(state.rows().all(|row| row.dirty()));
    clean(&mut state);
    feed(&mut t, "B");
    state.update(&mut t);
    state.update(&mut t);
    assert_eq!(state.dirty, Dirty::Partial);
    assert!(state.rows().next().unwrap().dirty());
}

/// Compare the visible data rather than allocation or stale nonvisual row flags.
fn compare_states(inc: &mut RenderState, fresh: &mut RenderState) {
    assert_eq!((inc.cols, inc.rows), (fresh.cols, fresh.rows));
    assert_eq!(inc.cursor, fresh.cursor);
    assert_eq!(inc.colors, fresh.colors);
    for (a, b) in inc.rows().zip(fresh.rows()) {
        assert_eq!(a.raw().styled(), b.raw().styled());
        assert_eq!(a.raw().grapheme(), b.raw().grapheme());
        for (a, b) in a.cells().zip(b.cells()) {
            assert_eq!(a.raw(), b.raw());
            assert_eq!(a.style(), b.style());
            assert_eq!(a.grapheme(), b.grapheme());
        }
    }
}

#[test]
fn incremental_updates_match_full_rebuild() {
    let mut t = Terminal::new(20, 8, 500).unwrap();
    let mut inc = RenderState::default();
    let ops = [
        "ABCDEFGHIJKLMNO",
        "x\r\nx\r\nx\r\n",
        "\x1b[3;5H",
        "\x1b[1m",
        "\x1b[48;2;30;60;90m",
        "\x1b[38;5;120m",
        "\x1b[0m",
        "\x1b[K",
        "\x1b[1K",
        "\x1b[J",
        "\x1b[2J",
        "\x1b[3J",
        "\x1b[2L",
        "\x1b[2M",
        "\x1b[2S",
        "\x1b[2T",
        "\x1b[2;7r",
        "\x1b[2@",
        "\x1b[2P",
        "\x1bM",
        "字👨\u{200d}👩\u{200d}👧",
        "\x1b[?1049h",
        "\x1b[?1049l",
        "\x1b#8",
    ];
    let mut seed = 0xb0ba_cafe_u32;
    for i in 0..300 {
        for _ in 0..=i % 6 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            feed(&mut t, ops[usize::try_from(seed).unwrap() % ops.len()]);
        }
        match i % 10 {
            0 => t.scroll_viewport(ScrollViewport::Delta(-3)),
            1 => t.scroll_viewport(ScrollViewport::Delta(2)),
            2 => t.scroll_viewport(ScrollViewport::Bottom),
            3 => t.scroll_viewport(ScrollViewport::Top),
            _ => {}
        }
        inc.update(&mut t);
        let mut fresh = RenderState::default();
        fresh.update(&mut t);
        compare_states(&mut inc, &mut fresh);
        clean(&mut inc);
    }
}

/// Every dynamic-colour route invalidates changes, but not repeated values.
fn dynamic_color_transition(set: &str, reset: &str, resetting: bool) {
    use maestro_terminal::color::Rgb;
    let mut t = terminal("hello");
    t.set_default_background_color(Some(Rgb { r: 1, g: 2, b: 3 }));
    t.set_default_foreground_color(Some(Rgb { r: 4, g: 5, b: 6 }));
    t.set_default_cursor_color(Some(Rgb { r: 7, g: 8, b: 9 }));
    let mut state = RenderState::default();
    state.update(&mut t);
    if resetting {
        feed(&mut t, set);
        state.update(&mut t);
    }
    let before = state.colors.clone();
    clean(&mut state);
    let bytes = if resetting { reset } else { set };
    feed(&mut t, bytes);
    state.update(&mut t);
    assert_ne!(state.colors, before);
    assert_eq!(state.dirty, Dirty::Full);
    assert!(state.rows().all(|row| row.dirty()));
    let after = state.colors.clone();
    clean(&mut state);
    feed(&mut t, bytes);
    state.update(&mut t);
    assert_eq!(state.colors, after);
    assert_eq!(state.dirty, Dirty::Clean);
    assert!(state.rows().all(|row| !row.dirty()));
}

#[test]
fn osc_foreground_set_invalidates() {
    dynamic_color_transition("\x1b]10;#112233\x07", "\x1b]110\x07", false);
}
#[test]
fn osc_foreground_reset_invalidates() {
    dynamic_color_transition("\x1b]10;#112233\x07", "\x1b]110\x07", true);
}
#[test]
fn osc_background_set_invalidates() {
    dynamic_color_transition("\x1b]11;#112233\x07", "\x1b]111\x07", false);
}
#[test]
fn osc_background_reset_invalidates() {
    dynamic_color_transition("\x1b]11;#112233\x07", "\x1b]111\x07", true);
}
#[test]
fn osc_cursor_set_invalidates() {
    dynamic_color_transition("\x1b]12;#112233\x07", "\x1b]112\x07", false);
}
#[test]
fn osc_cursor_reset_invalidates() {
    dynamic_color_transition("\x1b]12;#112233\x07", "\x1b]112\x07", true);
}
#[test]
fn keyboard_color_foreground_set_invalidates() {
    dynamic_color_transition(
        "\x1b]21;foreground=rgb:11/22/33\x1b\\",
        "\x1b]21;foreground=\x1b\\",
        false,
    );
}
#[test]
fn keyboard_color_foreground_reset_invalidates() {
    dynamic_color_transition(
        "\x1b]21;foreground=rgb:11/22/33\x1b\\",
        "\x1b]21;foreground=\x1b\\",
        true,
    );
}
#[test]
fn keyboard_color_background_set_invalidates() {
    dynamic_color_transition(
        "\x1b]21;background=rgb:11/22/33\x1b\\",
        "\x1b]21;background=\x1b\\",
        false,
    );
}
#[test]
fn keyboard_color_background_reset_invalidates() {
    dynamic_color_transition(
        "\x1b]21;background=rgb:11/22/33\x1b\\",
        "\x1b]21;background=\x1b\\",
        true,
    );
}
#[test]
fn keyboard_color_cursor_set_invalidates() {
    dynamic_color_transition(
        "\x1b]21;cursor=rgb:11/22/33\x1b\\",
        "\x1b]21;cursor=\x1b\\",
        false,
    );
}
#[test]
fn keyboard_color_cursor_reset_invalidates() {
    dynamic_color_transition(
        "\x1b]21;cursor=rgb:11/22/33\x1b\\",
        "\x1b]21;cursor=\x1b\\",
        true,
    );
}

#[test]
fn rendered_cells_preserve_combining_and_emoji_sequences() {
    const PAYLOAD: &str = "README 👨‍👩‍👧‍👦 🧑‍💻 ✅ ⚡ 漢字 café e\u{301} 🏳️‍🌈 🚀";
    let mut terminal = Terminal::new(80, 3, 100_000).unwrap();
    terminal
        .modes
        .set(maestro_terminal::Mode::GraphemeCluster, true);
    terminal.write(format!("{PAYLOAD}\r\n").as_bytes());
    let mut state = RenderState::default();
    state.update(&mut terminal);
    let row = state.rows().next().unwrap();
    let text: String = row.cells().map(|cell| cell.graphemes_utf8()).collect();
    assert_eq!(text, PAYLOAD);
}
