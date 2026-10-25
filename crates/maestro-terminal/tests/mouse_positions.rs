use maestro_terminal::{
    input::{
        key_mods::Mods,
        mouse::{MouseAction, MouseButton, MouseEvent, Pos},
        mouse_encode::{
            MouseEncoder,
            size::{CellSize, Padding, ScreenSize, Size},
        },
    },
    mouse::{Event, Format},
    point::Coordinate,
};

fn encoder() -> MouseEncoder {
    let mut encoder = MouseEncoder::new(Size {
        screen: ScreenSize {
            width: 100,
            height: 100,
        },
        cell: CellSize {
            width: 10,
            height: 20,
        },
        padding: Padding {
            left: 7,
            right: 3,
            top: 9,
            bottom: 1,
        },
    });
    encoder.set_event(Event::Any);
    encoder.set_format(Format::Sgr);
    encoder
}

fn event() -> MouseEvent {
    MouseEvent {
        button: Some(MouseButton::Left),
        pos: Pos { x: 17.0, y: 29.0 },
        ..MouseEvent::default()
    }
}

#[test]
fn cell_and_pixel_reports_remove_padding() {
    let mut encoder = encoder();
    assert_eq!(encoder.encode(&event()), b"\x1b[<0;2;2M");
    encoder.set_format(Format::SgrPixels);
    assert_eq!(encoder.encode(&event()), b"\x1b[<0;10;20M");
    let event = MouseEvent {
        pos: Pos { x: 6.5, y: 8.5 },
        ..event()
    };
    assert_eq!(encoder.encode(&event), b"\x1b[<0;-1;-1M");
}

#[test]
fn viewport_releases_are_unconditional_but_presses_require_drag_tracking() {
    let mut encoder = encoder();
    let mut event = MouseEvent {
        pos: Pos { x: 101.0, y: -1.0 },
        ..event()
    };
    encoder.set_event(Event::Normal);
    encoder.any_button_pressed = true;
    assert!(encoder.encode(&event).is_empty());
    event.action = MouseAction::Release;
    assert_eq!(encoder.encode(&event), b"\x1b[<0;9;1m");
    encoder.set_format(Format::SgrPixels);
    assert_eq!(encoder.encode(&event), b"\x1b[<0;94;-10m");
    event.action = MouseAction::Press;
    encoder.set_event(Event::Button);
    assert_eq!(encoder.encode(&event), b"\x1b[<0;94;-10M");
    encoder.any_button_pressed = false;
    assert!(encoder.encode(&event).is_empty());
}

#[test]
fn motion_dedupe_tracks_cells_not_fractional_pixels() {
    let mut encoder = encoder();
    encoder.last_cell = Some(None);
    let mut event = MouseEvent {
        action: MouseAction::Motion,
        ..event()
    };
    assert_eq!(encoder.encode(&event), b"\x1b[<32;2;2M");
    event.pos.x = 18.0;
    assert!(encoder.encode(&event).is_empty());
    event.pos.x = 27.0;
    assert_eq!(encoder.encode(&event), b"\x1b[<32;3;2M");
    encoder.last_cell = None;
    assert_eq!(encoder.encode(&event), b"\x1b[<32;3;2M");
    assert_eq!(encoder.encode(&event), b"\x1b[<32;3;2M");
}

#[test]
fn rejected_format_still_updates_enabled_cell_tracking() {
    let mut encoder = encoder();
    encoder.last_cell = Some(None);
    encoder.set_event(Event::None);
    assert!(encoder.encode(&event()).is_empty());
    assert_eq!(encoder.last_cell, Some(None));
    encoder.set_event(Event::Any);
    let event = MouseEvent {
        button: Some(MouseButton::Unknown),
        ..event()
    };
    assert!(encoder.encode(&event).is_empty());
    assert_eq!(encoder.last_cell, Some(Some(Coordinate { x: 1, y: 1 })));
}

#[test]
fn all_formats_keep_release_and_modifier_semantics() {
    let mut encoder = encoder();
    let event = MouseEvent {
        action: MouseAction::Release,
        button: Some(MouseButton::Right),
        mods: Mods::SHIFT
            .union(Mods::ALT)
            .union(Mods::CTRL)
            .union(Mods::SUPER)
            .union(Mods::CAPS_LOCK),
        ..event()
    };
    for (format, expected) in [
        (Format::X10, b"\x1b[M?\"\"".as_slice()),
        (Format::Utf8, b"\x1b[M?\"\""),
        (Format::Sgr, b"\x1b[<30;2;2m"),
        (Format::Urxvt, b"\x1b[63;2;2M"),
        (Format::SgrPixels, b"\x1b[<30;10;20m"),
    ] {
        encoder.set_format(format);
        assert_eq!(encoder.encode(&event), expected);
    }
}

#[test]
fn x10_accepts_the_last_encodable_cell_and_tracks_rejected_cells() {
    let mut encoder = encoder();
    let mut size = encoder.size();
    size.screen = ScreenSize {
        width: 1000,
        height: 1000,
    };
    size.padding = Padding::default();
    size.cell = CellSize {
        width: 1,
        height: 1,
    };
    encoder.set_size(size);
    encoder.last_cell = Some(None);
    encoder.set_format(Format::X10);
    let mut event = MouseEvent {
        pos: Pos { x: 222.0, y: 222.0 },
        ..event()
    };
    assert_eq!(encoder.encode(&event), b"\x1b[M \xff\xff");
    event.pos.y = 223.0;
    assert!(encoder.encode(&event).is_empty());
    assert_eq!(encoder.last_cell, Some(Some(Coordinate { x: 222, y: 223 })));
}
