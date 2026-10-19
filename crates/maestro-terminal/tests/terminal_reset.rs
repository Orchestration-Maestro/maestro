//! Full terminal reset retains host geometry but clears protocol state.

use super::support::terminal::{assert_text, cell};
use maestro_terminal::{
    Mode, Terminal, ansi::StatusDisplay, page::cell::SemanticContent, screen_set::Key,
    terminal::SwitchScreenMode,
};

fn styled() -> Terminal {
    use maestro_terminal::{color::Rgb, sgr::Attribute};
    let mut t = Terminal::new(80, 80, 0).unwrap();
    let rgb = Rgb {
        r: 255,
        g: 0,
        b: 127,
    };
    t.screens
        .active_mut()
        .set_attribute(Attribute::DirectColorFg(rgb))
        .unwrap();
    t.screens
        .active_mut()
        .set_attribute(Attribute::DirectColorBg(rgb))
        .unwrap();
    t
}
#[test]
fn editing_full_reset_with_a_non_empty_pen() {
    let mut t = styled();
    t.screens.active_mut().cursor.semantic_content = SemanticContent::Input;
    t.full_reset();
    assert_eq!(cell(&t, 0, 0).style_id(), 0);
    assert_eq!(t.screens.active().cursor.style_id, 0);
    assert_eq!(
        t.screens.active().cursor.semantic_content,
        SemanticContent::Output
    );
}
#[test]
fn editing_full_reset_hyperlink() {
    let mut t = Terminal::new(80, 80, 0).unwrap();
    t.screens
        .active_mut()
        .start_hyperlink(b"http://example.com", None)
        .unwrap();
    t.full_reset();
    assert_eq!(t.screens.active().cursor.hyperlink_id, 0);
    assert!(t.screens.active().cursor.hyperlink.is_none());
}
#[test]
fn editing_full_reset_with_a_non_empty_saved_cursor() {
    let mut t = styled();
    t.save_cursor();
    t.full_reset();
    assert_eq!(cell(&t, 0, 0).style_id(), 0);
    assert_eq!(t.screens.active().cursor.style_id, 0);
    assert!(t.screens.active().saved_cursor.is_none());
}
#[test]
fn editing_full_reset_origin_mode() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.set_cursor_pos(3, 5);
    t.modes.set(Mode::Origin, true);
    t.full_reset();
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (0, 0)
    );
    assert!(!t.modes.get(Mode::Origin));
}
#[test]
fn editing_full_reset_status_display() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.status_display = StatusDisplay::StatusLine;
    t.full_reset();
    assert_eq!(t.status_display, StatusDisplay::Main);
}
#[test]
fn editing_full_reset_clears_alt_screen_kitty_keyboard_state() {
    use maestro_terminal::kitty::key::Flags as KeyFlags;
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.switch_screen_mode(SwitchScreenMode::Mode1049, true)
        .unwrap();
    t.screens.active_mut().kitty_keyboard.push(KeyFlags::TRUE);
    t.switch_screen_mode(SwitchScreenMode::Mode1049, false)
        .unwrap();
    t.full_reset();
    assert!(t.screens.get(Key::Alternate).is_none());
}
#[test]
fn editing_full_reset_default_modes() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    assert!(t.modes.get(Mode::GraphemeCluster));
    t.full_reset();
    assert!(t.modes.get(Mode::GraphemeCluster));
}
#[test]
fn editing_full_reset_tracked_pins() {
    let mut t = Terminal::new(80, 80, 0).unwrap();
    let s = t.screens.active_mut();
    let pin = *s.pages.tracked_pin(s.cursor.page_pin).unwrap();
    let tracked = s.pages.track_pin(pin).unwrap();
    t.full_reset();
    let s = t.screens.active();
    assert!(
        s.pages
            .tracked_pin(tracked)
            .unwrap()
            .row_and_cell(&s.pages)
            .is_some()
    );
}
#[test]
fn reset_keeps_prompt_redraw_disabled_by_default() {
    use maestro_terminal::osc::semantic_prompt::Redraw;
    let mut t = Terminal::new(8, 3, 0).unwrap();
    assert_eq!(t.flags.shell_redraws_prompt, Redraw::False);
    t.flags.shell_redraws_prompt = Redraw::True;
    t.full_reset();
    assert_eq!(t.flags.shell_redraws_prompt, Redraw::False);
}
#[test]
fn reset_then_resize_preserves_unrequested_prompt() {
    use maestro_terminal::{
        osc::semantic_prompt::{Action, Command},
        page::row::SemanticPrompt,
    };
    let mut t = Terminal::new(8, 3, 0).unwrap();
    t.full_reset();
    t.semantic_prompt(&Command::init(Action::FreshLineNewPrompt))
        .unwrap();
    t.print_string("$ ").unwrap();
    assert_eq!(
        super::support::terminal::row(&t, 0).semantic_prompt(),
        SemanticPrompt::Prompt
    );
    t.resize(10, 3, 1, 1).unwrap();
    assert_text(&t, "$ ");
}
#[test]
fn reset_clears_metadata_and_retains_size_and_colors() {
    let mut t = Terminal::new(8, 3, 0).unwrap();
    t.resize(8, 3, 12, 20).unwrap();
    t.colors.background.default = Some(maestro_terminal::color::Rgb { r: 1, g: 2, b: 3 });
    t.colors.foreground.r#override = Some(maestro_terminal::color::Rgb { r: 4, g: 5, b: 6 });
    t.colors
        .palette
        .set(17, maestro_terminal::color::Rgb { r: 7, g: 8, b: 9 });
    let colors = t.colors.clone();
    t.set_title(b"title");
    t.set_pwd(b"raw%20dir");
    t.modes.set(Mode::EnableLeftAndRightMargin, true);
    t.set_top_and_bottom_margin(2, 3);
    t.set_left_and_right_margin(2, 5);
    t.switch_screen_mode(SwitchScreenMode::Mode1049, true)
        .unwrap();
    t.print_string("alternate").unwrap();
    t.full_reset();
    assert_eq!(t.screens.active_key(), Key::Primary);
    assert!(t.screens.get(Key::Alternate).is_none());
    assert_eq!(
        (t.cols(), t.rows(), t.width_px, t.height_px),
        (8, 3, 96, 60)
    );
    assert_eq!(t.colors, colors);
    assert_eq!((t.get_pwd(), t.get_title()), (None, None));
    assert_eq!(t.previous_char, None);
    assert!(t.flags.dirty.clear);
    assert_eq!(
        t.scrolling_region,
        maestro_terminal::terminal::ScrollingRegion {
            top: 0,
            bottom: 2,
            left: 0,
            right: 7
        }
    );
    assert_text(&t, "");
}
