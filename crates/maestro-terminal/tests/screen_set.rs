//! Lazy terminal screen ownership and independent lifetimes.

use maestro_terminal::{ScreenSet, screen::Options, screens::Key};

#[test]
fn screen_set_initializes_and_switches_lazily() {
    let mut set = ScreenSet::init(Options::default());
    assert_eq!(set.active_key(), Key::Primary);
    assert!(set.get(Key::Primary).is_some());
    assert!(set.get(Key::Alternate).is_none());
    assert_eq!(set.generation(Key::Primary), 0);
    assert_eq!(set.generation(Key::Alternate), 0);
    let opts = Options {
        cols: std::num::NonZeroU16::new(7).unwrap(),
        rows: std::num::NonZeroU16::new(3).unwrap(),
        max_scrollback: 1000,
    };
    let alternate = set.get_init(Key::Alternate, opts);
    assert_eq!((alternate.pages.cols(), alternate.pages.rows()), (7, 3));
    assert_eq!(set.generation(Key::Alternate), 0);
    set.switch_to(Key::Alternate).unwrap();
    assert_eq!(set.active_key(), Key::Alternate);
    let alternate = set.get_init(Key::Alternate, Options::default());
    assert_eq!((alternate.pages.cols(), alternate.pages.rows()), (7, 3));
    set.switch_to(Key::Primary).unwrap();
    assert_eq!(set.active_key(), Key::Primary);
    let primary = set.get(Key::Primary).unwrap();
    assert_eq!((primary.pages.cols(), primary.pages.rows()), (80, 24));
}

#[test]
fn screen_set_removal_advances_only_its_generation() {
    let mut set = ScreenSet::init(Options::default());
    assert_eq!(set.generation(Key::Primary), 0);
    assert_eq!(set.generation(Key::Alternate), 0);
    set.remove(Key::Alternate).unwrap();
    assert_eq!(set.generation(Key::Alternate), 0);
    set.get_init(Key::Alternate, Options::default());
    assert_eq!(set.generation(Key::Alternate), 0);
    let generation = set.generation(Key::Alternate);
    set.remove(Key::Alternate).unwrap();
    assert!(set.get(Key::Alternate).is_none());
    assert_eq!(set.generation(Key::Alternate), generation.wrapping_add(1));
    set.get_init(Key::Alternate, Options::default());
    assert_eq!(set.generation(Key::Alternate), generation.wrapping_add(1));
    assert_eq!(set.generation(Key::Primary), 0);
}

#[test]
fn screen_set_keeps_all_screen_state_independent() {
    let mut set = ScreenSet::init(Options::default());
    fill_screen(set.get_mut(Key::Primary).unwrap(), 1);
    assert!(set.get_mut(Key::Alternate).is_none());
    assert!(set.get(Key::Alternate).is_none());
    fill_screen(set.get_init(Key::Alternate, Options::default()), 2);
    set.switch_to(Key::Alternate).unwrap();
    check_screen(set.get(set.active_key()).unwrap(), 2);
    set.switch_to(Key::Primary).unwrap();
    check_screen(set.get(set.active_key()).unwrap(), 1);
    set.remove(Key::Alternate).unwrap();
    check_screen(set.get(Key::Primary).unwrap(), 1);
    assert_eq!(set.generation(Key::Primary), 0);
    assert_eq!(set.generation(Key::Alternate), 1);
    let alternate = set.get_init(Key::Alternate, Options::default());
    assert!(alternate.cursor.hyperlink.is_none());
    assert_eq!(alternate.cursor.x, 0);
    assert_eq!(alternate.cursor.style, maestro_terminal::Style::default());
    assert_eq!(
        alternate.charset,
        maestro_terminal::screen::CharsetState::default()
    );
    assert_eq!(
        alternate.kitty_keyboard.current(),
        maestro_terminal::kitty::key::Flags::DISABLED
    );
    check_screen(set.get(Key::Primary).unwrap(), 1);
    assert_eq!(set.generation(Key::Alternate), 1);
}

fn fill_screen(screen: &mut maestro_terminal::Screen, value: u8) {
    use maestro_terminal::{
        Cell, Charset, CharsetSlot, ansi::ProtectedMode, kitty::key::Flags, sgr::Attribute,
    };
    screen.cursor_horizontal_absolute(u16::from(value));
    screen
        .set_attribute(Attribute::Palette256Bg(value))
        .unwrap();
    screen
        .start_hyperlink(&[b'/', value], Some(&[value]))
        .unwrap();
    let pin = *screen.pages.tracked_pin(screen.cursor.page_pin).unwrap();
    *pin.row_and_cell_mut(&mut screen.pages).unwrap().1 = Cell::init(char::from(b'a' + value));
    screen.cursor_set_hyperlink().unwrap();
    screen.cursor.hyperlink_implicit_id = u32::from(value);
    screen.cursor.protected = value == 1;
    screen.cursor.pending_wrap = value == 2;
    screen.charset.gl = if value == 1 {
        CharsetSlot::G1
    } else {
        CharsetSlot::G3
    };
    screen.charset.charsets.set(
        CharsetSlot::G2,
        if value == 1 {
            Charset::Ascii
        } else {
            Charset::British
        },
    );
    screen.protected_mode = if value == 1 {
        ProtectedMode::Iso
    } else {
        ProtectedMode::Dec
    };
    screen.semantic_prompt.seen = value == 2;
    screen.kitty_keyboard.push(if value == 1 {
        Flags::DISAMBIGUATE
    } else {
        Flags::REPORT_EVENTS
    });
}

fn check_screen(screen: &maestro_terminal::Screen, value: u8) {
    use maestro_terminal::{links::HyperlinkId, style::Color};
    let pin = *screen.pages.tracked_pin(screen.cursor.page_pin).unwrap();
    assert_eq!(
        pin.row_and_cell(&screen.pages).unwrap().1.codepoint(),
        char::from(b'a' + value)
    );
    assert_eq!(screen.cursor.x, u16::from(value));
    assert_eq!(screen.cursor.style.bg_color, Color::Palette(value));
    let link = screen.cursor.hyperlink.as_ref().unwrap();
    assert_eq!(link.id, HyperlinkId::Explicit(vec![value]));
    assert_eq!(link.uri, vec![b'/', value]);
    assert_eq!(screen.cursor.hyperlink_implicit_id, u32::from(value));
    assert_eq!(screen.cursor.protected, value == 1);
    assert_eq!(screen.cursor.pending_wrap, value == 2);
    check_protocol_state(screen, value);
}

fn check_protocol_state(screen: &maestro_terminal::Screen, value: u8) {
    use maestro_terminal::{Charset, CharsetSlot, ansi::ProtectedMode, kitty::key::Flags};
    assert_eq!(
        screen.charset.gl,
        if value == 1 {
            CharsetSlot::G1
        } else {
            CharsetSlot::G3
        }
    );
    assert_eq!(
        screen.charset.charsets.get(CharsetSlot::G2),
        if value == 1 {
            Charset::Ascii
        } else {
            Charset::British
        }
    );
    assert_eq!(
        screen.protected_mode,
        if value == 1 {
            ProtectedMode::Iso
        } else {
            ProtectedMode::Dec
        }
    );
    assert_eq!(screen.semantic_prompt.seen, value == 2);
    assert_eq!(
        screen.kitty_keyboard.current(),
        if value == 1 {
            Flags::DISAMBIGUATE
        } else {
            Flags::REPORT_EVENTS
        }
    );
}

#[test]
fn screen_set_active_transitions_refuse_invalid_lifetimes() {
    use maestro_terminal::screens::ScreenSetError;
    let mut set = ScreenSet::init(Options::default());
    assert!(std::ptr::eq(set.active(), set.get(Key::Primary).unwrap()));
    assert_eq!(
        set.switch_to(Key::Alternate),
        Err(ScreenSetError::Uninitialized)
    );
    assert_eq!(set.active_key(), Key::Primary);
    assert_eq!(
        set.remove(Key::Primary),
        Err(ScreenSetError::PrimaryRemoval)
    );
    set.get_init(Key::Alternate, Options::default());
    set.switch_to(Key::Alternate).unwrap();
    assert!(std::ptr::eq(set.active(), set.get(Key::Alternate).unwrap()));
    set.active_mut().cursor.pending_wrap = true;
    assert!(set.get(Key::Alternate).unwrap().cursor.pending_wrap);
    assert_eq!(
        set.remove(Key::Alternate),
        Err(ScreenSetError::ActiveRemoval)
    );
    assert_eq!(set.generation(Key::Alternate), 0);
    assert_eq!(set.active_key(), Key::Alternate);
    set.switch_to(Key::Primary).unwrap();
    assert!(std::ptr::eq(set.active(), set.get(Key::Primary).unwrap()));
    set.remove(Key::Alternate).unwrap();
    assert_eq!(set.generation(Key::Alternate), 1);
    assert_eq!(set.active_key(), Key::Primary);
    assert!(set.get(Key::Alternate).is_none());
    set.remove(Key::Alternate).unwrap();
    assert_eq!(set.generation(Key::Alternate), 1);
}

#[test]
fn active_removal_preserves_selected_residency() {
    use maestro_terminal::screens::ScreenSetError;
    let mut set = ScreenSet::init(Options::default());
    set.get_init(Key::Alternate, Options::default());
    set.switch_to(Key::Alternate).unwrap();
    assert!(set.get(set.active_key()).is_some());
    assert_eq!(
        set.remove(Key::Alternate),
        Err(ScreenSetError::ActiveRemoval)
    );
    assert!(std::ptr::eq(
        set.active(),
        set.get(set.active_key()).unwrap()
    ));
}
