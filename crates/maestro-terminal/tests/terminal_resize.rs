//! Terminal resize policy over retained screen reflow.

use super::support::terminal::{assert_text, cell};
use maestro_terminal::{Mode, Terminal, terminal::DeccolmMode};

#[test]
fn editing_resize_less_cols_with_wide_char_then_print() {
    let mut t = Terminal::new(3, 3, 0).unwrap();
    t.print_string("x😀").unwrap();
    t.resize(2, 3, 1, 1).unwrap();
    t.set_cursor_pos(1, 2);
    t.print('😀').unwrap();
}
#[test]
fn editing_resize_with_left_and_right_margin_set() {
    let mut t = Terminal::new(70, 23, 0).unwrap();
    t.modes.set(Mode::EnableLeftAndRightMargin, true);
    t.print('0').unwrap();
    t.modes.set(Mode::EnableMode3, true);
    t.resize(70, 23, 1, 1).unwrap();
    t.set_left_and_right_margin(2, 0);
    t.print_repeat(1850).unwrap();
    t.modes.restore(Mode::EnableMode3);
    t.resize(70, 23, 1, 1).unwrap();
}
#[test]
fn editing_resize_with_wraparound_off() {
    let mut t = Terminal::new(4, 2, 0).unwrap();
    t.modes.set(Mode::Wraparound, false);
    t.print_string("0123").unwrap();
    t.resize(2, 2, 1, 1).unwrap();
    assert_text(&t, "01");
}
#[test]
fn editing_resize_with_wraparound_on() {
    let mut t = Terminal::new(4, 2, 0).unwrap();
    t.modes.set(Mode::Wraparound, true);
    t.print_string("0123").unwrap();
    t.resize(2, 2, 1, 1).unwrap();
    assert_text(&t, "01\n23");
}
#[test]
fn editing_deccolm_without_dec_mode_40() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.modes.set(Mode::Column132, true);
    t.deccolm(DeccolmMode::Cols132).unwrap();
    assert_eq!((t.cols(), t.rows()), (5, 5));
    assert!(!t.modes.get(Mode::Column132));
}
#[test]
fn editing_deccolm_unset() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.modes.set(Mode::EnableMode3, true);
    t.deccolm(DeccolmMode::Cols80).unwrap();
    assert_eq!((t.cols(), t.rows()), (80, 5));
}
#[test]
fn editing_deccolm_resets_pending_wrap() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABCDE").unwrap();
    assert!(t.screens.active().cursor.pending_wrap);
    t.modes.set(Mode::EnableMode3, true);
    t.deccolm(DeccolmMode::Cols80).unwrap();
    assert_eq!((t.cols(), t.rows()), (80, 5));
    assert!(!t.screens.active().cursor.pending_wrap);
}
#[test]
fn editing_deccolm_preserves_sgr_bg() {
    use maestro_terminal::{color::Rgb, sgr::Attribute};
    let mut t = Terminal::new(5, 5, 0).unwrap();
    let rgb = Rgb { r: 255, g: 0, b: 0 };
    t.screens
        .active_mut()
        .set_attribute(Attribute::DirectColorBg(rgb))
        .unwrap();
    t.modes.set(Mode::EnableMode3, true);
    t.deccolm(DeccolmMode::Cols80).unwrap();
    assert_eq!(
        cell(&t, 0, 0).content(),
        maestro_terminal::page::cell::Content::BgColorRgb(rgb)
    );
}
#[test]
fn editing_deccolm_resets_scroll_region() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.modes.set(Mode::EnableLeftAndRightMargin, true);
    t.set_top_and_bottom_margin(2, 3);
    t.set_left_and_right_margin(3, 5);
    t.modes.set(Mode::EnableMode3, true);
    t.deccolm(DeccolmMode::Cols80).unwrap();
    assert!(t.modes.get(Mode::EnableLeftAndRightMargin));
    assert_eq!(
        t.scrolling_region,
        maestro_terminal::terminal::ScrollingRegion {
            top: 0,
            bottom: 4,
            left: 0,
            right: 79
        }
    );
}

#[test]
fn editing_resize_with_high_unique_style_per_cell() {
    use maestro_terminal::{color::Rgb, sgr::Attribute};
    let mut t = Terminal::new(30, 30, 0).unwrap();
    for y in 0..30u8 {
        for x in 0..30u8 {
            t.set_cursor_pos(usize::from(y), usize::from(x));
            t.screens
                .active_mut()
                .set_attribute(Attribute::DirectColorBg(Rgb { r: x, g: y, b: 0 }))
                .unwrap();
            t.print('x').unwrap();
        }
    }
    t.resize(60, 30, 1, 1).unwrap();
}
#[test]
fn editing_resize_with_high_unique_style_per_cell_with_wrapping() {
    use maestro_terminal::{color::Rgb, sgr::Attribute};
    let mut t = Terminal::new(30, 30, 0).unwrap();
    for i in 0..900u16 {
        let [g, r] = i.to_le_bytes();
        t.screens
            .active_mut()
            .set_attribute(Attribute::DirectColorBg(Rgb { r, g, b: 0 }))
            .unwrap();
        t.print('x').unwrap();
    }
    t.resize(60, 30, 1, 1).unwrap();
}
#[test]
fn editing_resize_with_reflow_and_saved_cursor() {
    let mut t = Terminal::new(2, 3, 0).unwrap();
    t.print_string("1A2B").unwrap();
    t.set_cursor_pos(2, 2);
    assert_eq!(
        cell(
            &t,
            t.screens.active().cursor.x,
            u32::from(t.screens.active().cursor.y)
        )
        .codepoint(),
        'B'
    );
    assert_text(&t, "1A\n2B");
    t.save_cursor();
    t.resize(5, 3, 1, 1).unwrap();
    t.restore_cursor();
    assert_text(&t, "1A2B");
    assert_eq!(
        cell(
            &t,
            t.screens.active().cursor.x,
            u32::from(t.screens.active().cursor.y)
        )
        .codepoint(),
        'B'
    );
}
#[test]
fn editing_resize_with_reflow_and_saved_cursor_pending_wrap() {
    let mut t = Terminal::new(2, 3, 0).unwrap();
    t.print_string("1A2B").unwrap();
    assert_eq!(
        cell(
            &t,
            t.screens.active().cursor.x,
            u32::from(t.screens.active().cursor.y)
        )
        .codepoint(),
        'B'
    );
    assert_text(&t, "1A\n2B");
    t.save_cursor();
    t.resize(5, 3, 1, 1).unwrap();
    t.restore_cursor();
    assert_text(&t, "1A2B");
    t.print('X').unwrap();
    assert_text(&t, "1A2BX");
}
#[test]
fn editing_resize() {
    let mut t = Terminal::new(80, 24, 10_000).unwrap();
    t.resize(40, 12, 9, 18).unwrap();
    assert_eq!((t.cols(), t.rows()), (40, 12));
}
#[test]
fn editing_get_cols_and_rows() {
    let t = Terminal::new(80, 24, 0).unwrap();
    assert_eq!((t.cols(), t.rows()), (80, 24));
}
#[test]
fn editing_resize_updates_pixel_dimensions() {
    let mut t = Terminal::new(80, 24, 0).unwrap();
    t.resize(100, 40, 9, 18).unwrap();
    assert_eq!((t.width_px, t.height_px), (900, 720));
}
#[test]
fn editing_resize_pixel_overflow_saturates() {
    let mut t = Terminal::new(80, 24, 0).unwrap();
    t.resize(100, 40, u32::MAX, u32::MAX).unwrap();
    assert_eq!((t.width_px, t.height_px), (u32::MAX, u32::MAX));
}
#[test]
fn editing_resize_disables_synchronized_output() {
    let mut t = Terminal::new(80, 24, 0).unwrap();
    t.modes.set(Mode::SynchronizedOutput, true);
    t.resize(100, 40, 9, 18).unwrap();
    assert!(!t.modes.get(Mode::SynchronizedOutput));
}
#[test]
fn editing_resize_zero_cols() {
    let mut t = Terminal::new(80, 24, 0).unwrap();
    assert_eq!(
        t.resize(0, 40, 9, 18),
        Err(maestro_terminal::terminal::ResizeError::Size(
            maestro_terminal::SizeError
        ))
    );
}
#[test]
fn editing_resize_zero_rows() {
    let mut t = Terminal::new(80, 24, 0).unwrap();
    assert_eq!(
        t.resize(100, 0, 9, 18),
        Err(maestro_terminal::terminal::ResizeError::Size(
            maestro_terminal::SizeError
        ))
    );
}
#[test]
fn same_grid_resize_updates_only_host_geometry_and_sync() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    t.print_string("AB").unwrap();
    t.modes.set(Mode::EnableLeftAndRightMargin, true);
    t.set_top_and_bottom_margin(2, 4);
    t.set_left_and_right_margin(2, 8);
    let margins = t.scrolling_region;
    for (width, height, expected) in [
        (7, 9, (70, 45)),
        (u32::MAX, 3, (u32::MAX, 15)),
        (2, u32::MAX, (20, u32::MAX)),
    ] {
        t.flags.dirty.clear = false;
        t.modes.set(Mode::SynchronizedOutput, true);
        t.resize(10, 5, width, height).unwrap();
        assert_eq!((t.width_px, t.height_px), expected);
        assert!(!t.flags.dirty.clear);
        assert!(!t.modes.get(Mode::SynchronizedOutput));
        assert_eq!(t.scrolling_region, margins);
        assert_text(&t, "AB");
    }
    t.flags.dirty.clear = true;
    t.resize(10, 5, 1, 1).unwrap();
    assert!(t.flags.dirty.clear);
}
#[test]
fn zero_grid_resize_preserves_all_state() {
    use maestro_terminal::{
        screen_set::Key,
        terminal::{ResizeError, SwitchScreenMode},
    };
    let mut t = Terminal::new(10, 5, 0).unwrap();
    t.resize(10, 5, 7, 9).unwrap();
    t.switch_screen_mode(SwitchScreenMode::Mode47, true)
        .unwrap();
    t.print_string("AB").unwrap();
    t.modes.set(Mode::SynchronizedOutput, true);
    t.modes.set(Mode::EnableLeftAndRightMargin, true);
    t.set_top_and_bottom_margin(2, 4);
    t.set_left_and_right_margin(2, 8);
    t.flags.dirty.clear = false;
    let modes = t.modes.clone();
    let flags = t.flags;
    let margins = t.scrolling_region;
    for (cols, rows) in [(0, 5), (10, 0), (0, 0)] {
        assert_eq!(
            t.resize(cols, rows, 99, 101),
            Err(ResizeError::Size(maestro_terminal::SizeError))
        );
        assert_eq!(
            (t.cols(), t.rows(), t.width_px, t.height_px),
            (10, 5, 70, 45)
        );
        assert_eq!(t.modes, modes);
        assert_eq!(t.flags, flags);
        assert_eq!(t.scrolling_region, margins);
        assert_eq!(t.screens.active_key(), Key::Alternate);
        assert_text(&t, "AB");
    }
}
#[test]
fn zero_cell_pixels_use_one_pixel_geometry() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    for (width, height, expected) in [(0, 7, (12, 21)), (9, 0, (108, 3)), (0, 0, (12, 3))] {
        t.resize(12, 3, width, height).unwrap();
        assert_eq!((t.width_px, t.height_px), expected);
    }
}
#[test]
fn alternate_resize_does_not_reflow_soft_lines() {
    use maestro_terminal::{screen_set::Key, terminal::SwitchScreenMode};
    let mut t = Terminal::new(4, 3, 100_000).unwrap();
    t.print_string("ABCDEFGH").unwrap();
    t.switch_screen_mode(SwitchScreenMode::Mode1049, true)
        .unwrap();
    t.set_cursor_pos(1, 1);
    t.print_string("12345678").unwrap();
    t.resize(2, 3, 1, 1).unwrap();
    assert_text(&t, "12\n56");
    assert!(t.screens.active().no_scrollback);
    assert_eq!(t.screens.active().pages.history_rows(), 0);
    t.switch_screen(Key::Primary);
    assert_text(&t, "CD\nEF\nGH");
}

#[test]
fn editing_save_cursor_resize() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    t.set_cursor_pos(1, 10);
    t.save_cursor();
    t.resize(5, 5, 1, 1).unwrap();
    t.restore_cursor();
    t.print('X').unwrap();
    assert_text(&t, "    X");
}
