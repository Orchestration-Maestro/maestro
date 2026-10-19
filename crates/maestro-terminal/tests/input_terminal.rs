//! Input bytes from snapshots of the terminal's negotiated state.

use maestro_terminal::{
    Mode, Terminal,
    input::{
        key::{Key, KeyEvent},
        key_encode::{KeyEncoder, OptionAsAlt},
        key_mods::Mods,
    },
    kitty::key::Flags,
};

#[test]
fn key_encoder_from_terminal_preserves_terminal_defaults() {
    let terminal = Terminal::new(80, 24, 0).unwrap();
    let encoder = KeyEncoder::from_terminal(&terminal);
    assert!(!encoder.modes.get(Mode::CursorKeys));
    assert!(encoder.modes.get(Mode::AltEscPrefix));
    assert!(encoder.modes.get(Mode::IgnoreKeypadWithNumlock));
    assert_eq!(encoder.kitty_flags, Flags::DISABLED);
    assert_eq!(encoder.macos_option_as_alt, OptionAsAlt::False);
    assert!(!encoder.modify_other_keys_state_2);
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::ArrowUp,
            ..KeyEvent::default()
        }),
        b"\x1b[A"
    );
}

fn size() -> maestro_terminal::input::mouse_encode::size::Size {
    use maestro_terminal::input::mouse_encode::size::{CellSize, Padding, ScreenSize, Size};
    Size {
        screen: ScreenSize {
            width: 80,
            height: 24,
        },
        cell: CellSize {
            width: 1,
            height: 1,
        },
        padding: Padding::default(),
    }
}

#[test]
fn input_encoders_observe_terminal_modes() {
    use maestro_terminal::input::{
        mouse::{MouseButton, MouseEvent},
        mouse_encode::MouseEncoder,
    };
    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    terminal.write(b"\x1b[?1h\x1b[>1u\x1b[?1000h\x1b[?1006h");
    let key = KeyEvent {
        key: Key::KeyA,
        mods: Mods::CTRL.union(Mods::SHIFT),
        utf8: "A".into(),
        unshifted_codepoint: u32::from('a'),
        ..KeyEvent::default()
    };
    assert_eq!(
        KeyEncoder::from_terminal(&terminal).encode(&key),
        b"\x1b[97;6u"
    );
    let mut mouse = MouseEncoder::from_terminal(&terminal, size(), None);
    assert_eq!(
        mouse.encode(&MouseEvent {
            button: Some(MouseButton::Left),
            ..MouseEvent::default()
        }),
        b"\x1b[<0;1;1M"
    );
}

#[test]
fn key_snapshot_copies_all_legacy_modes_and_extended_modifiers() {
    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    terminal.write(b"\x1b[?1;66;67h\x1b[?1035;1036l\x1b[>4;2m");
    let encoder = KeyEncoder::from_terminal(&terminal);
    for mode in [Mode::CursorKeys, Mode::KeypadKeys, Mode::BackarrowKeyMode] {
        assert!(encoder.modes.get(mode));
    }
    assert!(!encoder.modes.get(Mode::IgnoreKeypadWithNumlock));
    assert!(!encoder.modes.get(Mode::AltEscPrefix));
    assert!(encoder.modify_other_keys_state_2);
    for (key, expected) in [
        (Key::ArrowUp, b"\x1bOA".as_slice()),
        (Key::Backspace, b"\x08"),
        (Key::Numpad1, b"\x1bOq"),
    ] {
        assert_eq!(
            encoder.encode(&KeyEvent {
                key,
                ..KeyEvent::default()
            }),
            expected
        );
    }
    let modified = KeyEvent {
        key: Key::KeyH,
        mods: Mods::CTRL.union(Mods::SHIFT),
        utf8: "H".into(),
        ..KeyEvent::default()
    };
    assert_eq!(encoder.encode(&modified), b"\x1b[27;6;72~");
    terminal.full_reset();
    drop(terminal);
    assert_eq!(encoder.encode(&modified), b"\x1b[27;6;72~");
}

#[test]
fn key_snapshot_uses_active_screen_protocol_stack() {
    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    terminal.write(b"\x1b[>31u");
    let primary = KeyEncoder::from_terminal(&terminal);
    assert_eq!(primary.kitty_flags, Flags::TRUE);
    terminal.write(b"\x1b[?1049h\x1b[>2u");
    let alternate = KeyEncoder::from_terminal(&terminal);
    assert_eq!(alternate.kitty_flags.int(), 2);
    terminal.write(b"\x1b[?1049l");
    assert_eq!(
        KeyEncoder::from_terminal(&terminal).kitty_flags,
        Flags::TRUE
    );
    assert_eq!(alternate.kitty_flags.int(), 2);
    assert_eq!(primary.kitty_flags, Flags::TRUE);
}

#[test]
fn mouse_snapshot_defaults_disable_reports_and_tracking() {
    use maestro_terminal::{
        input::{mouse::MouseEvent, mouse_encode::MouseEncoder},
        mouse::{Event, Format},
    };
    let terminal = Terminal::new(80, 24, 0).unwrap();
    let mut encoder = MouseEncoder::from_terminal(&terminal, size(), None);
    assert_eq!(encoder.event(), Event::None);
    assert_eq!(encoder.format(), Format::X10);
    assert_eq!(encoder.size(), size());
    assert!(!encoder.any_button_pressed);
    assert_eq!(encoder.last_cell, None);
    assert!(encoder.encode(&MouseEvent::default()).is_empty());
}

#[test]
fn mouse_snapshot_reads_event_and_format_modes() {
    use maestro_terminal::{
        input::mouse_encode::MouseEncoder,
        mouse::{Event, Format},
    };
    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    for (mode, expected) in [
        (9, Event::X10),
        (1000, Event::Normal),
        (1002, Event::Button),
        (1003, Event::Any),
    ] {
        terminal.write(format!("\x1b[?{mode}h").as_bytes());
        assert_eq!(
            MouseEncoder::from_terminal(&terminal, size(), None).event(),
            expected
        );
        terminal.write(format!("\x1b[?{mode}l").as_bytes());
    }
    for (mode, expected) in [
        (1005, Format::Utf8),
        (1006, Format::Sgr),
        (1015, Format::Urxvt),
        (1016, Format::SgrPixels),
    ] {
        terminal.write(format!("\x1b[?{mode}h").as_bytes());
        assert_eq!(
            MouseEncoder::from_terminal(&terminal, size(), None).format(),
            expected
        );
        terminal.write(format!("\x1b[?{mode}l").as_bytes());
    }
}

#[test]
fn mouse_snapshot_allows_size_and_format_overrides_without_borrowing() {
    use maestro_terminal::{
        input::{
            mouse::{MouseButton, MouseEvent, Pos},
            mouse_encode::MouseEncoder,
        },
        mouse::Format,
    };
    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    terminal.write(b"\x1b[?1003;1006h");
    let mut geometry = size();
    geometry.cell.width = 2;
    let mut encoder = MouseEncoder::from_terminal(&terminal, geometry, Some(Format::SgrPixels));
    terminal.full_reset();
    drop(terminal);
    assert_eq!(
        encoder.encode(&MouseEvent {
            button: Some(MouseButton::Left),
            pos: Pos { x: 4.0, y: 6.0 },
            ..MouseEvent::default()
        }),
        b"\x1b[<0;4;6M"
    );
    encoder.set_format(Format::Sgr);
    assert_eq!(
        encoder.encode(&MouseEvent {
            button: Some(MouseButton::Left),
            pos: Pos { x: 4.0, y: 6.0 },
            ..MouseEvent::default()
        }),
        b"\x1b[<0;3;7M"
    );
}
