use super::style::{PaletteOverrides, cell_color, cell_symbol};
use super::*;
use crate::render::frame::frame_from_buffer;
use maestro_app::daemon::{RgbColor, TerminalTheme};
use maestro_app::terminal::{Color as CellColor, RenderState, Rgb, Terminal, Wide};
use maestro_protocol::frames::{CellData, underline_style_from_modifier};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};

fn rgb(r: u8, g: u8, b: u8) -> Rgb {
    Rgb { r, g, b }
}

/// A pane whose emulator has already processed `bytes`.
fn pane(cols: u16, rows: u16, bytes: &str) -> PaneTerminal {
    let pane = PaneTerminal::new(Terminal::new(cols, rows, 0).unwrap());
    pane.seed_history_ansi(bytes);
    pane
}

/// The frame buffer of one pane drawn over a whole backend of its size.
fn draw(pane: &PaneTerminal, width: u16, height: u16) -> Buffer {
    let mut terminal = ratatui::Terminal::new(TestBackend::new(width, height)).unwrap();
    let completed = terminal
        .draw(|frame| render(pane, frame, Rect::new(0, 0, width, height), false))
        .unwrap();
    completed.buffer.clone()
}

/// Host colours with a known foreground and background.
fn known_host_theme() -> TerminalTheme {
    TerminalTheme {
        foreground: Some(RgbColor {
            r: 0xaa,
            g: 0xbb,
            b: 0xcc,
        }),
        background: Some(RgbColor {
            r: 0x11,
            g: 0x22,
            b: 0x33,
        }),
        ..TerminalTheme::default()
    }
}

/// Symbols of the first viewport row, with each cell's width class.
fn row_symbols(text: &str) -> Vec<(Wide, String)> {
    let mut terminal = Terminal::new(40, 1, 0).unwrap();
    terminal.write(text.as_bytes());
    let mut state = RenderState::default();
    state.update(&mut terminal);
    let row = state.rows().next().unwrap();
    row.cells()
        .map(|cell| {
            let mut symbol = String::new();
            cell_symbol(&cell, false, &mut symbol);
            (cell.raw().wide(), symbol)
        })
        .collect()
}

fn patch_rows(outcome: TerminalDirtyPatchOutcome) -> Vec<(u16, Vec<CellData>)> {
    match outcome {
        TerminalDirtyPatchOutcome::Patch(patch) => patch.rows,
        other => panic!("expected a dirty patch, got {other:?}"),
    }
}

#[test]
fn pane_render_has_no_palette_overrides_without_osc4() {
    let default = [rgb(1, 2, 3); 256];
    assert!(PaletteOverrides::new(&default, &default).is_none());
}

#[test]
fn pane_render_uses_rgb_for_redefined_palette_entries_only() {
    let default = [rgb(1, 2, 3); 256];
    let mut active = default;
    active[18] = rgb(169, 177, 214);
    let overrides = PaletteOverrides::new(&active, &default).unwrap();
    assert_eq!(
        cell_color(CellColor::Palette(18), Some(&overrides)),
        Some(Color::Rgb(169, 177, 214))
    );
    assert_eq!(
        cell_color(CellColor::Palette(19), Some(&overrides)),
        Some(Color::Indexed(19))
    );
    assert_eq!(
        cell_color(CellColor::Palette(18), None),
        Some(Color::Indexed(18))
    );

    let pane = pane(
        20,
        5,
        "\x1b]4;18;rgb:a9/b1/d6\x1b\\\x1b[38;5;18mA\x1b[38;5;19mB",
    );
    let buffer = draw(&pane, 20, 5);
    assert_eq!(buffer[(0, 0)].fg, Color::Rgb(169, 177, 214));
    assert_eq!(buffer[(1, 0)].fg, Color::Indexed(19));
}

#[test]
fn pane_render_keeps_direct_rgb_cells_unchanged_by_overrides() {
    let default = [rgb(1, 2, 3); 256];
    let mut active = default;
    active[18] = rgb(169, 177, 214);
    let overrides = PaletteOverrides::new(&active, &default).unwrap();
    assert_eq!(
        cell_color(CellColor::Rgb(rgb(122, 162, 247)), Some(&overrides)),
        Some(Color::Rgb(122, 162, 247))
    );
    assert_eq!(cell_color(CellColor::None, Some(&overrides)), None);
}

#[test]
fn pane_render_keeps_flag_grapheme_in_one_wide_cell() {
    let cells = row_symbols("🇧🇷");
    assert!(cells.contains(&(Wide::Wide, "🇧🇷".to_owned())), "{cells:?}");
}

#[test]
fn pane_render_keeps_zwj_family_in_one_wide_cell() {
    let family = "👨\u{200d}👩\u{200d}👧";
    let cells = row_symbols(family);
    assert!(
        cells.contains(&(Wide::Wide, family.to_owned())),
        "{cells:?}"
    );
}

#[test]
fn pane_render_keeps_halfwidth_voiced_kana() {
    let text = "ｱｲｳｴｵ ｶﾞｷﾞｸﾞｹﾞｺﾞ ﾊﾟﾋﾟﾌﾟﾍﾟﾎﾟ";
    let rendered: String = row_symbols(text)
        .into_iter()
        .map(|(_, symbol)| symbol)
        .collect();
    assert!(rendered.contains(text), "{rendered:?}");
}

#[test]
fn pane_render_leaves_voiced_kana_tail_empty() {
    let buffer = draw(&pane(20, 1, "ｶﾞZ"), 20, 1);
    assert_eq!(buffer[(0, 0)].symbol(), "ｶ\u{ff9e}");
    assert_eq!(buffer[(1, 0)].symbol(), "");
    assert_eq!(buffer[(2, 0)].symbol(), "Z");
}

#[test]
fn pane_dirty_patch_leaves_voiced_kana_tail_empty() {
    let pane = pane(20, 1, "");
    draw(&pane, 20, 1);
    pane.seed_history_ansi("ｶﾞZ");

    let rows = patch_rows(collect_dirty_patch(&pane, 20, 1));
    let row = &rows[0].1;
    assert_eq!(row[0].symbol, "ｶ\u{ff9e}");
    assert_eq!(row[1].symbol, "");
    assert_eq!(row[2].symbol, "Z");
}

#[test]
fn pane_dirty_patch_reports_clean_full_and_linked_rows() {
    let pane = pane(20, 3, "");
    assert_eq!(
        collect_dirty_patch(&pane, 20, 3),
        TerminalDirtyPatchOutcome::Fallback,
        "a never drawn pane needs a full frame"
    );
    draw(&pane, 20, 3);
    assert_eq!(
        collect_dirty_patch(&pane, 20, 3),
        TerminalDirtyPatchOutcome::Clean
    );

    pane.seed_history_ansi("ab\r\ncd");
    let rows = patch_rows(collect_dirty_patch(&pane, 4, 3));
    let rows: Vec<(u16, String)> = rows
        .into_iter()
        .map(|(y, cells)| (y, cells.into_iter().map(|cell| cell.symbol).collect()))
        .collect();
    assert_eq!(rows, [(0, "ab  ".to_owned()), (1, "cd  ".to_owned())]);
    assert_eq!(
        collect_dirty_patch(&pane, 4, 3),
        TerminalDirtyPatchOutcome::Clean
    );

    pane.seed_history_ansi("\x1b]8;;https://example.test\x1b\\L\x1b]8;;\x1b\\");
    assert_eq!(
        collect_dirty_patch(&pane, 4, 3),
        TerminalDirtyPatchOutcome::Fallback
    );
}

#[test]
fn pane_render_draws_seeded_history_on_next_frame() {
    let pane = PaneTerminal::new(Terminal::new(20, 5, 100).unwrap());
    pane.seed_history_ansi("restored history");
    let buffer = draw(&pane, 20, 5);
    let row: String = (0..16).map(|x| buffer[(x, 0)].symbol()).collect();
    assert_eq!(row, "restored history");
}

#[test]
fn pane_render_keeps_unknown_host_background_transparent() {
    let buffer = draw(&pane(20, 5, "hi"), 20, 5);
    assert_eq!(buffer[(0, 0)].symbol(), "h");
    assert_eq!(buffer[(0, 0)].style().fg, Some(Color::Reset));
    assert_eq!(buffer[(0, 0)].style().bg, Some(Color::Reset));
    assert_eq!(buffer[(2, 0)].symbol(), " ");
    assert_eq!(buffer[(2, 0)].style().fg, Some(Color::Reset));
    assert_eq!(buffer[(2, 0)].style().bg, Some(Color::Reset));
}

#[test]
fn pane_render_blanks_image_placeholders_when_graphics_enabled() {
    let pane = PaneTerminal::new(Terminal::new(20, 5, 0).unwrap());
    pane.set_kitty_graphics_enabled(true);
    pane.seed_history_ansi("before\u{10eeee}\u{0305}\u{0305}after");

    let buffer = draw(&pane, 20, 5);
    assert_eq!(buffer[(0, 0)].symbol(), "b");
    assert_eq!(buffer[(6, 0)].symbol(), " ");
    assert_eq!(buffer[(7, 0)].symbol(), "a");
    assert_eq!(pane.visible_text().lines().next(), Some("before after"));
    assert_eq!(pane.recent_text_snapshot(5).text, "before after\n");
}

#[test]
fn pane_render_keeps_explicit_foreground_with_unknown_host() {
    let buffer = draw(&pane(20, 5, "\x1b[38;2;68;85;102mhi\x1b[0m"), 20, 5);
    assert_eq!(buffer[(0, 0)].symbol(), "h");
    assert_eq!(
        buffer[(0, 0)].style().fg,
        Some(Color::Rgb(0x44, 0x55, 0x66))
    );
    assert_eq!(buffer[(2, 0)].symbol(), " ");
    assert_eq!(buffer[(2, 0)].style().fg, Some(Color::Reset));
}

#[test]
fn pane_render_keeps_explicit_background_with_unknown_host() {
    let buffer = draw(&pane(20, 5, "\x1b[48;2;68;85;102mhi\x1b[0m"), 20, 5);
    assert_eq!(buffer[(0, 0)].symbol(), "h");
    assert_eq!(
        buffer[(0, 0)].style().bg,
        Some(Color::Rgb(0x44, 0x55, 0x66))
    );
    assert_eq!(buffer[(2, 0)].symbol(), " ");
    assert_eq!(buffer[(2, 0)].style().bg, Some(Color::Reset));
}

#[test]
fn pane_render_keeps_palette_colors_indexed() {
    let bytes = "\x1b[31mR\x1b[0m \x1b[38;5;171mI\x1b[0m \x1b[48;5;4mB\x1b[0m \x1b[38;2;1;2;3mT";
    let buffer = draw(&pane(20, 5, bytes), 20, 5);
    assert_eq!(buffer[(0, 0)].symbol(), "R");
    assert_eq!(buffer[(0, 0)].style().fg, Some(Color::Indexed(1)));
    assert_eq!(buffer[(2, 0)].symbol(), "I");
    assert_eq!(buffer[(2, 0)].style().fg, Some(Color::Indexed(171)));
    assert_eq!(buffer[(4, 0)].symbol(), "B");
    assert_eq!(buffer[(4, 0)].style().bg, Some(Color::Indexed(4)));
    assert_eq!(buffer[(6, 0)].symbol(), "T");
    assert_eq!(buffer[(6, 0)].style().fg, Some(Color::Rgb(1, 2, 3)));
}

#[test]
fn pane_render_keeps_palette_background_fills() {
    let buffer = draw(&pane(20, 5, "\x1b[48;5;4m\x1b[K"), 20, 5);
    for x in 0..20 {
        assert_eq!(buffer[(x, 0)].symbol(), " ");
        assert_eq!(buffer[(x, 0)].style().bg, Some(Color::Indexed(4)));
    }
}

#[test]
fn pane_render_keeps_rgb_background_fills() {
    let buffer = draw(&pane(20, 5, "\x1b[48;2;17;34;51m\x1b[K"), 20, 5);
    for x in 0..20 {
        assert_eq!(buffer[(x, 0)].symbol(), " ");
        assert_eq!(buffer[(x, 0)].style().bg, Some(Color::Rgb(17, 34, 51)));
    }
}

#[test]
fn pane_render_keeps_underline_color() {
    let buffer = draw(&pane(20, 5, "\x1b[4m\x1b[58:2::17:34:51mU"), 20, 5);
    let style = buffer[(0, 0)].style();
    assert!(style.add_modifier.contains(Modifier::UNDERLINED));
    assert_eq!(style.underline_color, Some(Color::Rgb(17, 34, 51)));
}

#[test]
fn pane_dirty_patch_keeps_curly_underline_style() {
    let pane = pane(20, 5, "");
    draw(&pane, 20, 5);
    pane.seed_history_ansi("\x1b[4:3mU");

    let rows = patch_rows(collect_dirty_patch(&pane, 20, 5));
    let cell = &rows[0].1[0];
    assert_eq!(cell.symbol, "U");
    assert_eq!(underline_style_from_modifier(cell.modifier), 3);
}

#[test]
fn pane_full_frame_keeps_curly_underline_style() {
    let buffer = draw(&pane(20, 5, "\x1b[4:3mU"), 20, 5);
    let frame = frame_from_buffer(&buffer, None, &[]);
    assert_eq!(frame.cells[0].symbol, "U");
    assert_eq!(underline_style_from_modifier(frame.cells[0].modifier), 3);
}

#[test]
fn pane_render_keeps_known_host_background_transparent() {
    let pane = PaneTerminal::new(Terminal::new(20, 5, 0).unwrap());
    pane.apply_host_terminal_theme(known_host_theme());
    pane.seed_history_ansi("hi");

    let buffer = draw(&pane, 20, 5);
    assert_eq!(buffer[(0, 0)].symbol(), "h");
    assert_eq!(buffer[(0, 0)].style().fg, Some(Color::Reset));
    assert_eq!(buffer[(0, 0)].style().bg, Some(Color::Reset));
    assert_eq!(buffer[(2, 0)].symbol(), " ");
    assert_eq!(buffer[(2, 0)].style().fg, Some(Color::Reset));
    assert_eq!(buffer[(2, 0)].style().bg, Some(Color::Reset));
}

#[test]
fn pane_render_keeps_explicit_default_foreground_when_it_differs_from_host() {
    let pane = PaneTerminal::new(Terminal::new(20, 5, 0).unwrap());
    pane.apply_host_terminal_theme(known_host_theme());
    pane.seed_history_ansi("\x1b]10;rgb:44/55/66\x1b\\hi");

    let buffer = draw(&pane, 20, 5);
    let expected = Some(Color::Rgb(0x44, 0x55, 0x66));
    assert_eq!(buffer[(0, 0)].symbol(), "h");
    assert_eq!(buffer[(0, 0)].style().fg, expected);
    assert_eq!(buffer[(2, 0)].symbol(), " ");
    assert_eq!(buffer[(2, 0)].style().fg, expected);
}

#[test]
fn pane_render_keeps_explicit_default_background_when_it_differs_from_host() {
    let pane = PaneTerminal::new(Terminal::new(20, 5, 0).unwrap());
    pane.apply_host_terminal_theme(known_host_theme());
    pane.seed_history_ansi("\x1b]11;rgb:44/55/66\x1b\\hi");

    let buffer = draw(&pane, 20, 5);
    let expected = Some(Color::Rgb(0x44, 0x55, 0x66));
    assert_eq!(buffer[(0, 0)].symbol(), "h");
    assert_eq!(buffer[(0, 0)].style().bg, expected);
    assert_eq!(buffer[(2, 0)].symbol(), " ");
    assert_eq!(buffer[(2, 0)].style().bg, expected);
}

#[test]
fn pane_render_inverse_swaps_with_resolved_background() {
    let pane = PaneTerminal::new(Terminal::new(20, 5, 0).unwrap());
    pane.apply_host_terminal_theme(known_host_theme());
    pane.seed_history_ansi("\x1b[7mhi\x1b[27m");

    let buffer = draw(&pane, 20, 5);
    let cell = &buffer[(0, 0)];
    assert_eq!(cell.symbol(), "h");
    assert_eq!(cell.style().fg, Some(Color::Rgb(0x11, 0x22, 0x33)));
    assert_eq!(cell.style().bg, Some(Color::Rgb(0xaa, 0xbb, 0xcc)));
}

#[test]
fn pane_render_hides_invisible_text_in_its_background() {
    let buffer = draw(&pane(20, 5, "\x1b[8;48;5;4mh\x1b[0m"), 20, 5);
    assert_eq!(buffer[(0, 0)].symbol(), "h");
    assert_eq!(buffer[(0, 0)].style().fg, Some(Color::Indexed(4)));
    assert_eq!(buffer[(0, 0)].style().bg, Some(Color::Indexed(4)));
}

#[test]
fn pane_render_can_suppress_cursor_position() {
    let first = pane(20, 5, "left");
    let second = pane(20, 5, "r\r\nb");
    let mut terminal = ratatui::Terminal::new(TestBackend::new(40, 5)).unwrap();
    terminal
        .draw(|frame| {
            render(&first, frame, Rect::new(0, 0, 20, 5), true);
            render(&second, frame, Rect::new(20, 0, 20, 5), false);
        })
        .unwrap();
    terminal.backend_mut().assert_cursor_position((4, 0));
}

#[test]
fn pane_render_resets_cells_beyond_the_pane() {
    let pane = pane(2, 1, "ab");
    let buffer = draw(&pane, 4, 2);
    let rows: Vec<String> = (0..2)
        .map(|y| (0..4).map(|x| buffer[(x, y)].symbol()).collect())
        .collect();
    assert_eq!(rows, ["ab  ", "    "]);
}

#[test]
fn pane_render_keeps_explicit_default_background_with_unknown_host() {
    // The engine adopts child defaults once both are set.
    let bytes = "\x1b]10;rgb:aa/bb/cc\x1b\\\x1b]11;rgb:44/55/66\x1b\\hi";
    let buffer = draw(&pane(20, 5, bytes), 20, 5);
    let expected = Some(Color::Rgb(0x44, 0x55, 0x66));
    assert_eq!(buffer[(0, 0)].symbol(), "h");
    assert_eq!(buffer[(0, 0)].style().bg, expected);
    assert_eq!(buffer[(2, 0)].symbol(), " ");
    assert_eq!(buffer[(2, 0)].style().bg, expected);
}
