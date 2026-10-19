//! Construction and retained terminal state.

use maestro_terminal::{Mode, SizeError, Terminal};

#[test]
fn terminal_new_defaults_and_zero_dimensions() {
    for (cols, rows) in [(0, 24), (80, 0), (0, 0)] {
        assert!(matches!(Terminal::new(cols, rows, 0), Err(SizeError)));
    }
    let mut terminal = Terminal::new(80, 24, 10_000).unwrap();
    assert_eq!((terminal.cols(), terminal.rows()), (80, 24));
    assert_eq!(terminal.max_scrollback(), 10_000);
    assert_initial_flags(&terminal);
    assert!(terminal.modes.get(Mode::GraphemeCluster));
    assert!(terminal.modes.get(Mode::Wraparound));
    assert!(terminal.modes.get(Mode::CursorVisible));
    assert!(!terminal.modes.get(Mode::Insert));
    terminal.modes.set(Mode::GraphemeCluster, false);
    terminal.modes.reset();
    assert!(terminal.modes.get(Mode::GraphemeCluster));
    terminal.modes.restore(Mode::GraphemeCluster);
    assert!(!terminal.modes.get(Mode::GraphemeCluster));
    assert_initial_position(&terminal);
}

#[test]
fn terminal_reads_use_the_active_screen() {
    use maestro_terminal::{
        Cell, Coordinate, Point, kitty::key::Flags, screen::Options, screen_set::Key,
        terminal::ScrollViewport,
    };
    use std::num::NonZeroU16;
    let mut terminal = Terminal::new(8, 3, 200_000).unwrap();
    {
        let primary = terminal.screens.active_mut();
        primary.kitty_keyboard.push(Flags::DISAMBIGUATE);
        for _ in 0..4 {
            primary.pages.grow();
        }
        let pin = primary
            .pages
            .pin(Point::Active(Coordinate { x: 0, y: 0 }))
            .unwrap();
        *pin.row_and_cell_mut(&mut primary.pages).unwrap().1 = Cell::init('p');
    }
    let primary_total = terminal.total_rows();
    assert!(primary_total >= 7);
    terminal.scroll_viewport(ScrollViewport::Top);
    assert_eq!(terminal.scrollbar().offset, 0);
    assert_eq!(terminal.kitty_keyboard_flags(), Flags::DISAMBIGUATE);
    let alternate = terminal.screens.get_init(
        Key::Alternate,
        Options {
            cols: NonZeroU16::new(8).unwrap(),
            rows: NonZeroU16::new(3).unwrap(),
            max_scrollback: 200_000,
        },
    );
    alternate.kitty_keyboard.push(Flags::REPORT_EVENTS);
    alternate.pages.grow();
    let pin = alternate
        .pages
        .pin(Point::Active(Coordinate { x: 0, y: 0 }))
        .unwrap();
    *pin.row_and_cell_mut(&mut alternate.pages).unwrap().1 = Cell::init('a');
    let alternate_total = alternate.pages.total_rows();
    assert_ne!(alternate_total, primary_total);
    terminal.screens.switch_to(Key::Alternate).unwrap();
    assert_eq!(terminal.active_screen(), Key::Alternate);
    assert_eq!(terminal.total_rows(), alternate_total);
    assert_eq!(terminal.kitty_keyboard_flags(), Flags::REPORT_EVENTS);
    assert_alternate_viewport(&mut terminal, alternate_total);
    terminal.screens.switch_to(Key::Primary).unwrap();
    assert_eq!(terminal.active_screen(), Key::Primary);
    assert_eq!(terminal.total_rows(), primary_total);
    assert_eq!(terminal.kitty_keyboard_flags(), Flags::DISAMBIGUATE);
    assert_eq!(terminal.scrollbar().offset, 0);
    assert_eq!(
        (terminal.cols(), terminal.rows(), terminal.max_scrollback()),
        (8, 3, 200_000)
    );
    assert_screen_contents(&terminal);
}

#[test]
fn terminal_effective_colors_select_override_then_default() {
    use maestro_terminal::color::Rgb;
    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    let foreground = Rgb { r: 1, g: 2, b: 3 };
    let cursor = Rgb { r: 4, g: 5, b: 6 };
    let override_color = Rgb { r: 7, g: 8, b: 9 };
    assert_eq!(terminal.effective_foreground_color(), None);
    assert_eq!(terminal.effective_cursor_color(), None);
    terminal.colors.foreground.default = Some(foreground);
    assert_eq!(terminal.effective_foreground_color(), Some(foreground));
    assert_eq!(terminal.effective_cursor_color(), None);
    terminal.colors.cursor.default = Some(cursor);
    assert_eq!(terminal.effective_cursor_color(), Some(cursor));
    terminal.colors.foreground.set(override_color);
    assert_eq!(terminal.effective_foreground_color(), Some(override_color));
    assert_eq!(terminal.effective_cursor_color(), Some(cursor));
    terminal.colors.cursor.set(override_color);
    assert_eq!(terminal.effective_cursor_color(), Some(override_color));
    assert_eq!(terminal.colors.foreground.default, Some(foreground));
    assert_eq!(terminal.colors.cursor.default, Some(cursor));
}

#[test]
fn terminal_palette_replacement_keeps_overrides() {
    use maestro_terminal::color::{DEFAULT, Rgb};
    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    let explicit = Rgb {
        r: 10,
        g: 11,
        b: 12,
    };
    let replacement = Rgb {
        r: 20,
        g: 21,
        b: 22,
    };
    terminal.colors.palette.set(0, explicit);
    let mut defaults = DEFAULT;
    defaults[0] = replacement;
    defaults[1] = replacement;
    terminal.set_default_palette(&defaults);
    assert!(terminal.flags.dirty.palette);
    assert_eq!(terminal.default_palette()[0], replacement);
    assert_eq!(terminal.default_palette()[1], replacement);
    assert_eq!(terminal.colors.palette.current[0], explicit);
    assert_eq!(terminal.colors.palette.current[1], replacement);
    assert!(terminal.colors.palette.mask.is_set(0));
    assert!(!terminal.colors.palette.mask.is_set(1));
    terminal.flags.dirty.palette = false;
    terminal.set_default_palette(&DEFAULT);
    assert!(terminal.flags.dirty.palette);
    assert_eq!(terminal.colors.palette.current[0], explicit);
    assert_eq!(terminal.colors.palette.current[1], DEFAULT[1]);
    assert_eq!(terminal.default_palette(), DEFAULT);
}

#[test]
fn terminal_new_invalid_value() {
    assert!(matches!(Terminal::new(0, 24, 10_000), Err(SizeError)));
    assert!(matches!(Terminal::new(80, 0, 10_000), Err(SizeError)));
}

#[test]
fn terminal_mode_get_and_mode_set() {
    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    let cursor = Mode::from_number(25, false).unwrap();
    assert!(terminal.modes.get(cursor));
    terminal.modes.set(cursor, false);
    assert!(!terminal.modes.get(cursor));
    let insert = Mode::from_number(4, true).unwrap();
    assert!(!terminal.modes.get(insert));
    terminal.modes.set(insert, true);
    assert!(terminal.modes.get(insert));
}

#[test]
fn terminal_mode_get_unknown_mode() {
    let terminal = Terminal::new(80, 24, 0).unwrap();
    assert_eq!(
        Mode::from_number(9999, false).map(|mode| terminal.modes.get(mode)),
        None
    );
}

#[test]
fn terminal_mode_set_unknown_mode() {
    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    let before = terminal.modes.clone();
    assert_eq!(
        Mode::from_number(9999, false).map(|mode| terminal.modes.set(mode, true)),
        None
    );
    assert_eq!(terminal.modes, before);
}

#[test]
fn terminal_get_cursor_visible() {
    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    assert!(terminal.modes.get(Mode::CursorVisible));
    terminal.modes.set(Mode::CursorVisible, false);
    assert!(!terminal.modes.get(Mode::CursorVisible));
}

#[test]
fn terminal_get_active_screen() {
    let terminal = Terminal::new(80, 24, 0).unwrap();
    assert_eq!(
        terminal.active_screen(),
        maestro_terminal::screen_set::Key::Primary
    );
}

#[test]
fn terminal_get_total_rows() {
    let terminal = Terminal::new(80, 24, 10_000).unwrap();
    assert!(terminal.total_rows() >= 24);
}

#[test]
fn terminal_grid_ref_out_of_bounds() {
    use maestro_terminal::{Coordinate, Point};
    let terminal = Terminal::new(80, 24, 0).unwrap();
    assert!(
        terminal
            .screens
            .active()
            .pages
            .pin(Point::Active(Coordinate { x: 100, y: 0 }))
            .is_none()
    );
}

#[test]
fn terminal_set_and_get_color_palette() {
    use maestro_terminal::color::{DEFAULT, Rgb};
    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    assert_eq!(terminal.colors.palette.current[0], DEFAULT[0]);
    let mut palette = DEFAULT;
    palette[0] = Rgb {
        r: 0x12,
        g: 0x34,
        b: 0x56,
    };
    terminal.set_default_palette(&palette);
    assert_eq!(terminal.colors.palette.current[0], palette[0]);
    terminal.set_default_palette(&DEFAULT);
    assert_eq!(terminal.colors.palette.current[0], DEFAULT[0]);
}

#[test]
fn terminal_get_color_palette_default_vs_current() {
    use maestro_terminal::color::{DEFAULT, Rgb};
    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    let mut palette = DEFAULT;
    palette[0] = Rgb {
        r: 0x12,
        g: 0x34,
        b: 0x56,
    };
    terminal.set_default_palette(&palette);
    let explicit = Rgb {
        r: 255,
        g: 255,
        b: 255,
    };
    terminal.colors.palette.set(0, explicit);
    assert_eq!(terminal.colors.palette.current[0], explicit);
    assert_eq!(terminal.default_palette()[0], palette[0]);
}

#[test]
fn terminal_set_color_sets_dirty_flag() {
    use maestro_terminal::color::Rgb;
    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    let color = Rgb {
        r: 255,
        g: 255,
        b: 255,
    };
    terminal.flags.dirty.palette = false;
    terminal.set_default_foreground_color(Some(color));
    assert!(terminal.flags.dirty.palette);
    assert_eq!(terminal.effective_foreground_color(), Some(color));
    terminal.flags.dirty.palette = false;
    terminal.set_default_cursor_color(Some(color));
    assert!(terminal.flags.dirty.palette);
    assert_eq!(terminal.effective_cursor_color(), Some(color));
    terminal.flags.dirty.palette = false;
    terminal.set_default_background_color(Some(color));
    assert!(terminal.flags.dirty.palette);
    assert_eq!(terminal.colors.background.get(), Some(color));
    terminal.colors.foreground.set(Rgb::default());
    terminal.flags.dirty.palette = false;
    terminal.set_default_foreground_color(None);
    assert!(terminal.flags.dirty.palette);
    assert_eq!(terminal.colors.foreground.default, None);
    assert_eq!(terminal.effective_foreground_color(), Some(Rgb::default()));
    terminal.set_default_cursor_color(None);
    terminal.set_default_background_color(None);
    assert_eq!(terminal.effective_cursor_color(), None);
    assert_eq!(terminal.colors.background.get(), None);
}

/// Verify the host-facing construction defaults independently of mode reset.
fn assert_initial_flags(terminal: &Terminal) {
    assert_eq!((terminal.width_px, terminal.height_px), (0, 0));
    assert!(!terminal.flags.dirty.palette);
    assert!(!terminal.flags.dirty.clear);
    assert!(terminal.flags.focused);
    assert!(!terminal.flags.modify_other_keys_2);
    assert_eq!(
        terminal.flags.mouse_event,
        maestro_terminal::mouse::Event::None
    );
    assert_eq!(
        terminal.flags.mouse_format,
        maestro_terminal::mouse::Format::X10
    );
}

/// Verify viewport selection did not change either screen's retained content.
fn assert_screen_contents(terminal: &Terminal) {
    use maestro_terminal::{Coordinate, Point, screen_set::Key};
    for (key, expected) in [(Key::Primary, 'p'), (Key::Alternate, 'a')] {
        let screen = terminal.screens.get(key).unwrap();
        let pin = screen
            .pages
            .pin(Point::Active(Coordinate { x: 0, y: 0 }))
            .unwrap();
        assert_eq!(
            pin.row_and_cell(&screen.pages).unwrap().1.codepoint(),
            expected
        );
    }
}

/// Verify the initial scrolling boundaries and cursor position.
fn assert_initial_position(terminal: &Terminal) {
    assert_eq!(terminal.scrolling_region.top, 0);
    assert_eq!(terminal.scrolling_region.bottom, 23);
    assert_eq!(terminal.scrolling_region.left, 0);
    assert_eq!(terminal.scrolling_region.right, 79);
    assert_eq!(terminal.previous_char, None);
    assert_eq!(terminal.screens.active().cursor.x, 0);
    assert_eq!(terminal.screens.active().cursor.y, 0);
}

/// Verify all viewport destinations use the selected alternate screen.
fn assert_alternate_viewport(terminal: &mut Terminal, alternate_total: usize) {
    use maestro_terminal::terminal::ScrollViewport;
    terminal.scroll_viewport(ScrollViewport::Bottom);
    assert_eq!(terminal.scrollbar().offset, alternate_total - 3);
    terminal.scroll_viewport(ScrollViewport::Delta(-1));
    assert_eq!(terminal.scrollbar().offset, 0);
    terminal.scroll_viewport(ScrollViewport::Row(usize::MAX));
    assert_eq!(terminal.scrollbar().offset, alternate_total - 3);
}
