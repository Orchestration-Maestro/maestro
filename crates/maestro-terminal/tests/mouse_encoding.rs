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
            width: 1000,
            height: 1000,
        },
        cell: CellSize {
            width: 1,
            height: 1,
        },
        padding: Padding::default(),
    })
}

fn event() -> MouseEvent {
    MouseEvent {
        button: Some(MouseButton::Left),
        ..MouseEvent::default()
    }
}

#[test]
fn x10_press_left() {
    let mut encoder = encoder();
    encoder.set_event(Event::X10);
    encoder.set_format(Format::X10);
    let event = MouseEvent {
        action: MouseAction::Press,
        button: Some(MouseButton::Left),
        mods: Mods::SHIFT.union(Mods::ALT).union(Mods::CTRL),
        pos: Pos { x: 0.0, y: 0.0 },
    };
    assert_eq!(encoder.encode(&event), b"\x1b[M !!");
}

#[test]
fn x10_ignores_release() {
    let mut encoder = encoder();
    encoder.set_event(Event::X10);
    encoder.set_format(Format::X10);
    let event = MouseEvent {
        action: MouseAction::Release,
        button: Some(MouseButton::Left),
        mods: Mods::default(),
        pos: Pos { x: 0.0, y: 0.0 },
    };
    assert_eq!(encoder.encode(&event), b"");
}

#[test]
fn normal_ignores_motion() {
    let mut encoder = encoder();
    encoder.set_event(Event::Normal);
    encoder.set_format(Format::Sgr);
    let event = MouseEvent {
        action: MouseAction::Motion,
        button: Some(MouseButton::Left),
        mods: Mods::default(),
        pos: Pos { x: 0.0, y: 0.0 },
    };
    assert_eq!(encoder.encode(&event), b"");
}

#[test]
fn button_mode_requires_button() {
    let mut encoder = encoder();
    encoder.set_event(Event::Button);
    encoder.set_format(Format::Sgr);
    let event = MouseEvent {
        action: MouseAction::Motion,
        button: None,
        mods: Mods::default(),
        pos: Pos { x: 0.0, y: 0.0 },
    };
    assert_eq!(encoder.encode(&event), b"");
}

#[test]
fn sgr_release_keeps_button_identity() {
    let mut encoder = encoder();
    encoder.set_event(Event::Any);
    encoder.set_format(Format::Sgr);
    let event = MouseEvent {
        action: MouseAction::Release,
        button: Some(MouseButton::Right),
        mods: Mods::default(),
        pos: Pos { x: 4.0, y: 5.0 },
    };
    assert_eq!(encoder.encode(&event), b"\x1b[<2;5;6m");
}

#[test]
fn sgr_motion_with_no_button() {
    let mut encoder = encoder();
    encoder.set_event(Event::Any);
    encoder.set_format(Format::Sgr);
    let event = MouseEvent {
        action: MouseAction::Motion,
        button: None,
        mods: Mods::default(),
        pos: Pos { x: 1.0, y: 2.0 },
    };
    assert_eq!(encoder.encode(&event), b"\x1b[<35;2;3M");
}

#[test]
fn urxvt_with_modifiers() {
    let mut encoder = encoder();
    encoder.set_event(Event::Any);
    encoder.set_format(Format::Urxvt);
    let event = MouseEvent {
        action: MouseAction::Press,
        button: Some(MouseButton::Left),
        mods: Mods::SHIFT.union(Mods::ALT).union(Mods::CTRL),
        pos: Pos { x: 2.0, y: 3.0 },
    };
    assert_eq!(encoder.encode(&event), b"\x1b[60;3;4M");
}

#[test]
fn utf8_encodes_large_coordinates() {
    let mut encoder = encoder();
    encoder.set_event(Event::Any);
    encoder.set_format(Format::Utf8);
    let event = MouseEvent {
        action: MouseAction::Press,
        button: Some(MouseButton::Left),
        mods: Mods::default(),
        pos: Pos { x: 300.0, y: 400.0 },
    };
    assert_eq!(encoder.encode(&event), "\x1b[M \u{14d}\u{1b1}".as_bytes());
}

#[test]
fn x10_coordinate_limit() {
    let mut encoder = encoder();
    encoder.set_event(Event::X10);
    encoder.set_format(Format::X10);
    let event = MouseEvent {
        action: MouseAction::Press,
        button: Some(MouseButton::Left),
        mods: Mods::default(),
        pos: Pos { x: 223.0, y: 0.0 },
    };
    assert_eq!(encoder.encode(&event), b"");
}

#[test]
fn urxvt_release_uses_legacy_button_3_encoding() {
    let mut encoder = encoder();
    encoder.set_event(Event::Any);
    encoder.set_format(Format::Urxvt);
    let event = MouseEvent {
        action: MouseAction::Release,
        button: Some(MouseButton::Right),
        mods: Mods::default(),
        pos: Pos { x: 2.0, y: 3.0 },
    };
    assert_eq!(encoder.encode(&event), b"\x1b[35;3;4M");
}

#[test]
fn unsupported_button_is_ignored() {
    let mut encoder = encoder();
    encoder.set_event(Event::Any);
    encoder.set_format(Format::Sgr);
    let event = MouseEvent {
        action: MouseAction::Press,
        button: Some(MouseButton::Ten),
        mods: Mods::default(),
        pos: Pos { x: 1.0, y: 1.0 },
    };
    assert_eq!(encoder.encode(&event), b"");
}

#[test]
fn sgr_pixels_uses_terminal_space_cursor_coordinates() {
    let mut encoder = encoder();
    encoder.set_event(Event::Any);
    encoder.set_format(Format::SgrPixels);
    let event = MouseEvent {
        action: MouseAction::Press,
        button: Some(MouseButton::Left),
        mods: Mods::default(),
        pos: Pos { x: 10.0, y: 20.0 },
    };
    assert_eq!(encoder.encode(&event), b"\x1b[<0;10;20M");
}

#[test]
fn sgr_pixels_release_keeps_button_identity() {
    let mut encoder = encoder();
    encoder.set_event(Event::Any);
    encoder.set_format(Format::SgrPixels);
    let event = MouseEvent {
        action: MouseAction::Release,
        button: Some(MouseButton::Right),
        mods: Mods::default(),
        pos: Pos { x: 10.0, y: 20.0 },
    };
    assert_eq!(encoder.encode(&event), b"\x1b[<2;10;20m");
}

#[test]
fn position_exactly_at_viewport_boundary_is_encoded_in_final_cell() {
    let mut encoder = encoder();
    encoder.set_event(Event::Any);
    encoder.set_format(Format::Sgr);
    encoder.set_size(Size {
        screen: ScreenSize {
            width: 10,
            height: 10,
        },
        cell: CellSize {
            width: 2,
            height: 2,
        },
        padding: Padding::default(),
    });
    let event = MouseEvent {
        action: MouseAction::Press,
        button: Some(MouseButton::Left),
        mods: Mods::default(),
        pos: Pos { x: 10.0, y: 10.0 },
    };
    assert_eq!(encoder.encode(&event), b"\x1b[<0;5;5M");
}

#[test]
fn outside_viewport_motion_with_no_pressed_button_is_ignored() {
    let mut encoder = encoder();
    encoder.set_event(Event::Any);
    encoder.set_format(Format::Sgr);
    let event = MouseEvent {
        action: MouseAction::Motion,
        button: Some(MouseButton::Left),
        mods: Mods::default(),
        pos: Pos { x: -1.0, y: -1.0 },
    };
    assert_eq!(encoder.encode(&event), b"");
}

#[test]
fn outside_viewport_motion_with_pressed_button_is_reported() {
    let mut encoder = encoder();
    encoder.set_event(Event::Any);
    encoder.set_format(Format::Sgr);
    encoder.any_button_pressed = true;
    let event = MouseEvent {
        action: MouseAction::Motion,
        button: Some(MouseButton::Left),
        mods: Mods::default(),
        pos: Pos { x: -1.0, y: -1.0 },
    };
    assert_eq!(encoder.encode(&event), b"\x1b[<32;1;1M");
}

#[test]
fn sgr_wheel_button_mappings() {
    let mut encoder = encoder();
    encoder.set_event(Event::Any);
    encoder.set_format(Format::Sgr);
    for (button, code) in [
        (MouseButton::Four, 64),
        (MouseButton::Five, 65),
        (MouseButton::Six, 66),
        (MouseButton::Seven, 67),
        (MouseButton::Eight, 128),
        (MouseButton::Nine, 129),
    ] {
        assert_eq!(
            encoder.encode(&MouseEvent {
                button: Some(button),
                ..event()
            }),
            format!("\x1b[<{code};1;1M").as_bytes()
        );
    }
}

#[test]
fn motion_is_deduped_by_last_cell_except_sgr_pixels() {
    let mut encoder = encoder();
    encoder.set_event(Event::Any);
    encoder.set_format(Format::Sgr);
    encoder.last_cell = Some(None);
    let event = MouseEvent {
        action: MouseAction::Motion,
        pos: Pos { x: 5.0, y: 6.0 },
        ..event()
    };
    assert_eq!(encoder.encode(&event), b"\x1b[<32;6;7M");
    assert!(encoder.encode(&event).is_empty());
    encoder.set_format(Format::SgrPixels);
    assert_eq!(encoder.encode(&event), b"\x1b[<32;5;6M");
    encoder.set_format(Format::Sgr);
    encoder.last_cell = Some(None);
    assert_eq!(encoder.encode(&event), b"\x1b[<32;6;7M");
}

fn reporting(mode: Event, actions: [bool; 3], button: Option<MouseButton>) {
    let mut encoder = encoder();
    encoder.set_event(mode);
    encoder.set_format(Format::Sgr);
    for (action, expected) in [
        MouseAction::Press,
        MouseAction::Release,
        MouseAction::Motion,
    ]
    .into_iter()
    .zip(actions)
    {
        assert_eq!(
            !encoder
                .encode(&MouseEvent {
                    action,
                    button,
                    ..event()
                })
                .is_empty(),
            expected,
            "{mode:?} {action:?} {button:?}"
        );
    }
}

#[test]
fn none_mode_never_reports() {
    reporting(Event::None, [false, false, false], Some(MouseButton::Left));
}

#[test]
fn x10_reports_only_primary_button_presses() {
    for button in [MouseButton::Left, MouseButton::Middle, MouseButton::Right] {
        reporting(Event::X10, [true, false, false], Some(button));
    }
    reporting(Event::X10, [false, false, false], Some(MouseButton::Four));
    reporting(Event::X10, [false, false, false], None);
}

#[test]
fn normal_reports_press_and_release_not_motion() {
    reporting(Event::Normal, [true, true, false], Some(MouseButton::Left));
}

#[test]
fn button_mode_requires_a_button() {
    reporting(Event::Button, [true, true, true], Some(MouseButton::Left));
    reporting(Event::Button, [false, false, false], None);
}

#[test]
fn any_mode_reports_everything() {
    reporting(Event::Any, [true, true, true], Some(MouseButton::Left));
    reporting(Event::Any, [true, true, true], None);
}

fn tracked_motion() -> (MouseEncoder, MouseEvent) {
    let mut encoder = encoder();
    encoder.set_event(Event::Any);
    encoder.set_format(Format::Sgr);
    encoder.last_cell = Some(None);
    let event = MouseEvent {
        action: MouseAction::Motion,
        pos: Pos { x: 1.0, y: 1.0 },
        ..event()
    };
    assert_eq!(encoder.encode(&event), b"\x1b[<32;2;2M");
    assert!(encoder.encode(&event).is_empty());
    (encoder, event)
}

#[test]
fn changing_format_invalidates_the_motion_cell() {
    let (mut encoder, event) = tracked_motion();
    encoder.set_format(Format::Sgr);
    assert!(encoder.encode(&event).is_empty());
    encoder.set_format(Format::Urxvt);
    assert_eq!(encoder.encode(&event), b"\x1b[64;2;2M");
    assert!(encoder.encode(&event).is_empty());
}

#[test]
fn changing_reporting_class_invalidates_the_motion_cell() {
    let (mut encoder, event) = tracked_motion();
    encoder.set_event(Event::Any);
    assert!(encoder.encode(&event).is_empty());
    encoder.set_event(Event::Button);
    assert_eq!(encoder.encode(&event), b"\x1b[<32;2;2M");
    assert!(encoder.encode(&event).is_empty());
}

#[test]
fn setting_size_invalidates_the_motion_cell_even_when_equal() {
    let (mut encoder, event) = tracked_motion();
    let mut size = encoder.size();
    size.screen.width += 1;
    encoder.set_size(size);
    assert_eq!(encoder.encode(&event), b"\x1b[<32;2;2M");
    assert!(encoder.encode(&event).is_empty());
    encoder.set_size(size);
    assert_eq!(encoder.encode(&event), b"\x1b[<32;2;2M");
    assert!(encoder.encode(&event).is_empty());
}
