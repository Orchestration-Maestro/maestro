//! Mouse observations through owned records and encoded reports.

use maestro_terminal::{
    input::{
        key_mods::Mods,
        mouse_encode::{
            MouseEncoder,
            dimensions::{CellSize, Padding, ScreenSize, Size},
        },
        pointer::{MouseAction, MouseButton, MouseEvent, Pos},
    },
    mouse::{Event, Format},
};

fn encoder() -> MouseEncoder {
    MouseEncoder::new(Size {
        screen: ScreenSize {
            width: 800,
            height: 600,
        },
        cell: CellSize {
            width: 10,
            height: 20,
        },
        padding: Padding::default(),
    })
}

#[test]
fn mouse_event_defaults_and_disabled_encoder_emit_nothing() {
    let event = MouseEvent::default();
    assert_eq!(event.action, MouseAction::Press);
    assert_eq!(event.button, None);
    assert!(event.mods.empty());
    assert_eq!(event.pos, Pos { x: 0.0, y: 0.0 });
    let mut encoder = encoder();
    assert_eq!(encoder.event(), Event::None);
    assert_eq!(encoder.format(), Format::X10);
    assert!(!encoder.any_button_pressed);
    assert_eq!(encoder.last_cell, None);
    assert!(encoder.encode(&event).is_empty());
}

#[test]
fn mouse_event_fields_roundtrip_to_motion_bytes() {
    let mut event = MouseEvent {
        action: MouseAction::Motion,
        button: Some(MouseButton::Left),
        mods: Mods::SHIFT.union(Mods::CTRL),
        pos: Pos { x: 12.5, y: -4.0 },
    };
    assert_eq!(event.pos, Pos { x: 12.5, y: -4.0 });
    assert_eq!(event.mods, Mods::SHIFT.union(Mods::CTRL));
    let mut encoder = encoder();
    encoder.set_event(Event::Any);
    encoder.set_format(Format::Sgr);
    encoder.any_button_pressed = true;
    assert_eq!(encoder.encode(&event), b"\x1b[<52;2;1M");
    event.button = None;
    event.pos.y = 4.0;
    assert_eq!(encoder.encode(&event), b"\x1b[<55;2;1M");
}

#[test]
fn mouse_sgr_press_left_and_motion_dedupe_reset() {
    let mut encoder = encoder();
    encoder.set_event(Event::Any);
    encoder.set_format(Format::Sgr);
    encoder.last_cell = Some(None);
    let mut event = MouseEvent {
        button: Some(MouseButton::Left),
        ..MouseEvent::default()
    };
    assert_eq!(encoder.encode(&event), b"\x1b[<0;1;1M");
    event.action = MouseAction::Motion;
    event.pos = Pos { x: 5.0, y: 6.0 };
    assert!(encoder.encode(&event).is_empty());
    encoder.last_cell = Some(None);
    assert_eq!(encoder.encode(&event), b"\x1b[<32;1;1M");
    assert!(encoder.encode(&event).is_empty());
    encoder.last_cell = Some(None);
    assert_eq!(encoder.encode(&event), b"\x1b[<32;1;1M");
}
