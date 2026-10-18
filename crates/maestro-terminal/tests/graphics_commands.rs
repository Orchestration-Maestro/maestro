//! Image command behaviour.
use maestro_terminal::kitty::graphics::command::*;

#[test]
fn payload_decoded_length_is_exact() {
    assert_eq!(
        Parser::parse_string(b";aGVsbG8gd29ybGQ=")
            .unwrap()
            .data
            .len(),
        11
    );
}

#[test]
fn transmission_retains_geometry() {
    assert_eq!(
        Parser::parse_string("f=24,s=10,v=20".as_bytes()).unwrap(),
        Command {
            control: Control::Transmit(Transmission {
                format: Format::Rgb,
                medium: Medium::Direct,
                width: 10,
                height: 20,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: false
            }),
            quiet: Quiet::No,
            data: vec![]
        }
    );
}

/// One typed source-record input and its complete expected outcome.
struct CommandCase {
    /// Complete command bytes as UTF-8 text.
    input: &'static str,
    /// Entire selected record, quiet policy and payload, or the exact error.
    expected: Result<(Control, Quiet, &'static [u8]), ParseError>,
}
/// Compare complete records without dropping payloads, errors or fields.
fn assert_cases(cases: &[CommandCase]) {
    for case in cases {
        match case.expected {
            Ok((control, quiet, data)) => assert_eq!(
                Parser::parse_string(case.input.as_bytes()).unwrap(),
                Command {
                    control,
                    quiet,
                    data: data.to_vec()
                }
            ),
            Err(error) => assert_eq!(Parser::parse_string(case.input.as_bytes()), Err(error)),
        }
    }
}

#[test]
fn payload_bytes_preserve_the_message() {
    assert_eq!(
        Parser::parse_string(b";aGVsbG8gd29ybGQ=").unwrap().data,
        b"hello world"
    );
}

#[test]
fn slice_feed_agrees_with_byte_feed() {
    let input = b"f=24,s=10,v=20;aGVsbG8gd29ybGQ=";
    let mut bytes = Parser::init(1024);
    for &byte in input {
        bytes.feed(byte).unwrap();
    }
    let mut slice = Parser::init(1024);
    slice.feed_slice(input).unwrap();
    assert_eq!(bytes.complete().unwrap(), slice.complete().unwrap());
}

#[test]
fn slice_feed_retains_partial_state() {
    let input = b"f=24,s=10,v=20;aGVsbG8gd29ybGQ=";
    let expected = Parser::parse_string(input).unwrap();
    for split in 0..=input.len() {
        let mut parser = Parser::init(1024);
        parser.feed_slice(&input[..split]).unwrap();
        parser.feed_slice(&input[split..]).unwrap();
        assert_eq!(parser.complete().unwrap(), expected, "split {split}");
    }
}

#[test]
fn slice_feed_enforces_payload_budget() {
    let mut parser = Parser::init(4);
    parser.feed_slice(b"f=24;QQ").unwrap();
    parser.feed_slice(b"==").unwrap();
    assert_eq!(parser.feed(b' '), Err(ParseError::OutOfMemory));
    assert_eq!(parser.complete().unwrap().data, b"A");
    let mut parser = Parser::init(4);
    parser.feed_slice(b"f=24;ab").unwrap();
    assert_eq!(parser.feed_slice(b"cde"), Err(ParseError::OutOfMemory));
}

#[test]
fn query_retains_transmission_fields() {
    assert_cases(QUERY_RETAINS_TRANSMISSION_FIELDS_CASES);
}
const QUERY_RETAINS_TRANSMISSION_FIELDS_CASES: &[CommandCase] = &[CommandCase {
    input: "i=31,s=1,v=1,a=q,t=d,f=24;QUFBQQ",
    expected: Ok((
        Control::Query(Transmission {
            format: Format::Rgb,
            medium: Medium::Direct,
            width: 1,
            height: 1,
            size: 0,
            offset: 0,
            image_id: 31,
            image_number: 0,
            placement_id: 0,
            compression: Compression::None,
            more_chunks: false,
        }),
        Quiet::No,
        &[65, 65, 65, 65],
    )),
}];

#[test]
fn display_retains_virtual_cell_geometry() {
    assert_cases(DISPLAY_RETAINS_VIRTUAL_CELL_GEOMETRY_CASES);
}
const DISPLAY_RETAINS_VIRTUAL_CELL_GEOMETRY_CASES: &[CommandCase] = &[CommandCase {
    input: "a=p,U=1,i=31,c=80,r=120",
    expected: Ok((
        Control::Display(Display {
            image_id: 31,
            image_number: 0,
            placement_id: 0,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            x_offset: 0,
            y_offset: 0,
            columns: 80,
            rows: 120,
            cursor_movement: CursorMovement::After,
            virtual_placement: true,
            parent_id: 0,
            parent_placement_id: 0,
            horizontal_offset: 0,
            vertical_offset: 0,
            z: 0,
        }),
        Quiet::No,
        &[],
    )),
}];

#[test]
fn cell_delete_retains_coordinates() {
    assert_cases(CELL_DELETE_RETAINS_COORDINATES_CASES);
}
const CELL_DELETE_RETAINS_COORDINATES_CASES: &[CommandCase] = &[CommandCase {
    input: "a=d,d=p,x=3,y=4",
    expected: Ok((
        Control::Delete(Delete::IntersectCell {
            delete: false,
            x: 3,
            y: 4,
        }),
        Quiet::No,
        &[],
    )),
}];

#[test]
fn animation_records_keep_only_parsed_fields() {
    assert_cases(ANIMATION_RECORDS_KEEP_ONLY_PARSED_FIELDS_CASES);
}
const ANIMATION_RECORDS_KEEP_ONLY_PARSED_FIELDS_CASES: &[CommandCase] = &[
    CommandCase {
        input: "a=f,x=1,y=2,c=3,r=4,z=-1,X=1,Y=67305985,i=90,I=91,p=92",
        expected: Ok((
            Control::TransmitAnimationFrame(AnimationFrameLoading {
                x: 1,
                y: 2,
                create_frame: 3,
                edit_frame: 4,
                gap_ms: 4_294_967_295,
                composition_mode: CompositionMode::Overwrite,
                background: Background {
                    r: 1,
                    g: 2,
                    b: 3,
                    a: 4,
                },
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=c,c=1,r=2,x=3,y=4,w=5,h=6,X=7,Y=8,C=1,i=90",
        expected: Ok((
            Control::ComposeAnimation(AnimationFrameComposition {
                frame: 1,
                edit_frame: 2,
                x: 3,
                y: 4,
                width: 5,
                height: 6,
                left_edge: 7,
                top_edge: 8,
                composition_mode: CompositionMode::Overwrite,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=a,s=3,r=2,z=-1,c=4,v=5,i=90",
        expected: Ok((
            Control::ControlAnimation(AnimationControl {
                action: AnimationAction::Run,
                frame: 2,
                gap_ms: 4_294_967_295,
                current_frame: 4,
                loops: 5,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=f;",
        expected: Ok((
            Control::TransmitAnimationFrame(AnimationFrameLoading {
                x: 0,
                y: 0,
                create_frame: 0,
                edit_frame: 0,
                gap_ms: 0,
                composition_mode: CompositionMode::AlphaBlend,
                background: Background {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 0,
                },
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=c;",
        expected: Ok((
            Control::ComposeAnimation(AnimationFrameComposition {
                frame: 0,
                edit_frame: 0,
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                left_edge: 0,
                top_edge: 0,
                composition_mode: CompositionMode::AlphaBlend,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=a;",
        expected: Ok((
            Control::ControlAnimation(AnimationControl {
                action: AnimationAction::Invalid,
                frame: 0,
                gap_ms: 0,
                current_frame: 0,
                loops: 0,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=a,s=0;",
        expected: Ok((
            Control::ControlAnimation(AnimationControl {
                action: AnimationAction::Invalid,
                frame: 0,
                gap_ms: 0,
                current_frame: 0,
                loops: 0,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=a,s=1;",
        expected: Ok((
            Control::ControlAnimation(AnimationControl {
                action: AnimationAction::Stop,
                frame: 0,
                gap_ms: 0,
                current_frame: 0,
                loops: 0,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=a,s=2;",
        expected: Ok((
            Control::ControlAnimation(AnimationControl {
                action: AnimationAction::RunWait,
                frame: 0,
                gap_ms: 0,
                current_frame: 0,
                loops: 0,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=a,s=3;",
        expected: Ok((
            Control::ControlAnimation(AnimationControl {
                action: AnimationAction::Run,
                frame: 0,
                gap_ms: 0,
                current_frame: 0,
                loops: 0,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=f,X=2;",
        expected: Err(ParseError::InvalidFormat),
    },
    CommandCase {
        input: "a=c,C=2;",
        expected: Err(ParseError::InvalidFormat),
    },
    CommandCase {
        input: "a=a,s=4;",
        expected: Err(ParseError::InvalidFormat),
    },
];

#[test]
fn combined_transmission_preserves_both_records() {
    assert_cases(COMBINED_TRANSMISSION_PRESERVES_BOTH_RECORDS_CASES);
}
const COMBINED_TRANSMISSION_PRESERVES_BOTH_RECORDS_CASES: &[CommandCase] = &[CommandCase {
    input: "a=T,f=32,t=d,s=2,v=3,S=24,O=4,i=5,I=6,p=7,o=z,m=2,x=8,y=9,w=10,h=11,X=12,Y=13,c=14,r=15,C=1,U=1,P=16,Q=17,H=-18,V=-19,z=-20,q=2",
    expected: Ok((
        Control::TransmitAndDisplay {
            transmission: Transmission {
                format: Format::Rgba,
                medium: Medium::Direct,
                width: 2,
                height: 3,
                size: 24,
                offset: 4,
                image_id: 5,
                image_number: 6,
                placement_id: 7,
                compression: Compression::ZlibDeflate,
                more_chunks: true,
            },
            display: Display {
                image_id: 5,
                image_number: 6,
                placement_id: 7,
                x: 8,
                y: 9,
                width: 10,
                height: 11,
                x_offset: 12,
                y_offset: 13,
                columns: 14,
                rows: 15,
                cursor_movement: CursorMovement::None,
                virtual_placement: true,
                parent_id: 16,
                parent_placement_id: 17,
                horizontal_offset: -18,
                vertical_offset: -19,
                z: -20,
            },
        },
        Quiet::Failures,
        &[],
    )),
}];

#[test]
fn response_without_identity_writes_nothing() {
    let mut bytes = Vec::new();
    Response {
        placement_id: 99,
        ..Response::default()
    }
    .encode(&mut bytes)
    .unwrap();
    assert!(bytes.is_empty());
}

#[test]
fn lowercase_range_keeps_endpoints() {
    assert_eq!(
        Parser::parse_string(b"a=d,d=r,x=3,y=4").unwrap().control,
        Control::Delete(Delete::Range {
            delete: false,
            first: 3,
            last: 4
        })
    );
}

#[test]
fn uppercase_range_keeps_endpoints() {
    assert_eq!(
        Parser::parse_string(b"a=d,d=R,x=5,y=11").unwrap().control,
        Control::Delete(Delete::Range {
            delete: true,
            first: 5,
            last: 11
        })
    );
}

#[test]
fn response_with_id_has_exact_bytes() {
    let mut bytes = Vec::new();
    Response {
        id: 4,
        ..Response::default()
    }
    .encode(&mut bytes)
    .unwrap();
    assert_eq!(bytes, b"\x1b_Gi=4;OK\x1b\\");
}

#[test]
fn response_with_number_has_exact_bytes() {
    let mut bytes = Vec::new();
    Response {
        image_number: 4,
        ..Response::default()
    }
    .encode(&mut bytes)
    .unwrap();
    assert_eq!(bytes, b"\x1b_GI=4;OK\x1b\\");
}

#[test]
fn response_with_both_identities_keeps_key_order() {
    let mut bytes = Vec::new();
    Response {
        id: 12,
        image_number: 4,
        ..Response::default()
    }
    .encode(&mut bytes)
    .unwrap();
    assert_eq!(bytes, b"\x1b_Gi=12,I=4;OK\x1b\\");
}

#[test]
fn response_placement_and_message_are_exact() {
    let mut response = Response {
        id: 7,
        image_number: 8,
        placement_id: 9,
        message: b"ENOENT:missing".to_vec(),
    };
    let mut bytes = Vec::new();
    response.encode(&mut bytes).unwrap();
    assert_eq!(bytes, b"\x1b_Gi=7,I=8,p=9;ENOENT:missing\x1b\\");
    assert!(!response.ok());
    response.message = b"ok".to_vec();
    assert!(!response.ok());
    assert!(Response::default().ok());
    assert!(
        Response {
            placement_id: 9,
            message: b"failure".to_vec(),
            ..Response::default()
        }
        .empty()
    );
    let original = std::io::Error::from_raw_os_error(123);
    assert_eq!(
        response
            .encode(&mut FailingWriter(Some(original)))
            .unwrap_err()
            .raw_os_error(),
        Some(123)
    );
}

#[test]
fn command_data_moves_without_affecting_control() {
    for (action, transmission, display) in [
        (b'q', true, false),
        (b't', true, false),
        (b'T', true, true),
        (b'p', false, true),
        (b'd', false, false),
        (b'f', false, false),
        (b'a', false, false),
        (b'c', false, false),
    ] {
        let mut command =
            Parser::parse_string(&[b'a', b'=', action, b';', b'Q', b'Q', b'=', b'=']).unwrap();
        let control = command.control;
        assert_eq!(command.to_owned_data(), b"A");
        assert!(command.data.is_empty());
        assert_eq!(command.control, control);
        assert_eq!(command.transmission().is_some(), transmission);
        assert_eq!(command.display().is_some(), display);
    }
}

#[test]
fn payload_only_uses_transmission_defaults() {
    assert_cases(PAYLOAD_ONLY_USES_TRANSMISSION_DEFAULTS_CASES);
}
const PAYLOAD_ONLY_USES_TRANSMISSION_DEFAULTS_CASES: &[CommandCase] = &[CommandCase {
    input: ";QUFBQQ",
    expected: Ok((
        Control::Transmit(Transmission {
            format: Format::Rgba,
            medium: Medium::Direct,
            width: 0,
            height: 0,
            size: 0,
            offset: 0,
            image_id: 0,
            image_number: 0,
            placement_id: 0,
            compression: Compression::None,
            more_chunks: false,
        }),
        Quiet::No,
        &[65, 65, 65, 65],
    )),
}];

#[test]
fn long_unknown_keys_leave_known_fields_intact() {
    assert_cases(LONG_UNKNOWN_KEYS_LEAVE_KNOWN_FIELDS_INTACT_CASES);
}
const LONG_UNKNOWN_KEYS_LEAVE_KNOWN_FIELDS_INTACT_CASES: &[CommandCase] = &[CommandCase {
    input: "f=24,s=10,v=20,hello=world",
    expected: Ok((
        Control::Transmit(Transmission {
            format: Format::Rgb,
            medium: Medium::Direct,
            width: 10,
            height: 20,
            size: 0,
            offset: 0,
            image_id: 0,
            image_number: 0,
            placement_id: 0,
            compression: Compression::None,
            more_chunks: false,
        }),
        Quiet::No,
        &[],
    )),
}];

#[test]
fn oversized_values_are_ignored() {
    assert_cases(OVERSIZED_VALUES_ARE_IGNORED_CASES);
}
const OVERSIZED_VALUES_ARE_IGNORED_CASES: &[CommandCase] = &[CommandCase {
    input: "f=24,s=10,v=2000000000000000000000000000000000000000",
    expected: Ok((
        Control::Transmit(Transmission {
            format: Format::Rgb,
            medium: Medium::Direct,
            width: 10,
            height: 0,
            size: 0,
            offset: 0,
            image_id: 0,
            image_number: 0,
            placement_id: 0,
            compression: Compression::None,
            more_chunks: false,
        }),
        Quiet::No,
        &[],
    )),
}];

#[test]
fn large_negative_layer_is_preserved() {
    assert_cases(LARGE_NEGATIVE_LAYER_IS_PRESERVED_CASES);
}
const LARGE_NEGATIVE_LAYER_IS_PRESERVED_CASES: &[CommandCase] = &[CommandCase {
    input: "a=p,i=1,z=-2000000000",
    expected: Ok((
        Control::Display(Display {
            image_id: 1,
            image_number: 0,
            placement_id: 0,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            x_offset: 0,
            y_offset: 0,
            columns: 0,
            rows: 0,
            cursor_movement: CursorMovement::After,
            virtual_placement: false,
            parent_id: 0,
            parent_placement_id: 0,
            horizontal_offset: 0,
            vertical_offset: 0,
            z: -2_000_000_000,
        }),
        Quiet::No,
        &[],
    )),
}];

#[test]
fn signed_display_offsets_keep_their_values() {
    for value in [i32::MIN, -1, i32::MAX] {
        for key in ["z", "H", "V"] {
            let cmd = Parser::parse_string(format!("a=p,{key}={value}").as_bytes()).unwrap();
            let d = cmd.display().unwrap();
            assert_eq!(
                match key {
                    "z" => d.z,
                    "H" => d.horizontal_offset,
                    _ => d.vertical_offset,
                },
                value
            );
        }
    }
    assert_cases(SIGNED_DISPLAY_OFFSETS_KEEP_THEIR_VALUES_CASES);
}
const SIGNED_DISPLAY_OFFSETS_KEEP_THEIR_VALUES_CASES: &[CommandCase] = &[
    CommandCase {
        input: "a=p,z=-1,H=-2,V=-3",
        expected: Ok((
            Control::Display(Display {
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                x_offset: 0,
                y_offset: 0,
                columns: 0,
                rows: 0,
                cursor_movement: CursorMovement::After,
                virtual_placement: false,
                parent_id: 0,
                parent_placement_id: 0,
                horizontal_offset: -2,
                vertical_offset: -3,
                z: -1,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=p,z=-2147483648,H=2147483647,V=0",
        expected: Ok((
            Control::Display(Display {
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                x_offset: 0,
                y_offset: 0,
                columns: 0,
                rows: 0,
                cursor_movement: CursorMovement::After,
                virtual_placement: false,
                parent_id: 0,
                parent_placement_id: 0,
                horizontal_offset: 2_147_483_647,
                vertical_offset: 0,
                z: -2_147_483_648,
            }),
            Quiet::No,
            &[],
        )),
    },
];

#[test]
fn numeric_control_values_keep_wire_rules() {
    assert_cases(NUMERIC_CONTROL_VALUES_KEEP_WIRE_RULES_CASES);
}
const NUMERIC_CONTROL_VALUES_KEEP_WIRE_RULES_CASES: &[CommandCase] = &[
    CommandCase {
        input: "a=p,i=4294967295",
        expected: Ok((
            Control::Display(Display {
                image_id: 4_294_967_295,
                image_number: 0,
                placement_id: 0,
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                x_offset: 0,
                y_offset: 0,
                columns: 0,
                rows: 0,
                cursor_movement: CursorMovement::After,
                virtual_placement: false,
                parent_id: 0,
                parent_placement_id: 0,
                horizontal_offset: 0,
                vertical_offset: 0,
                z: 0,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=p,i=A",
        expected: Ok((
            Control::Display(Display {
                image_id: 65,
                image_number: 0,
                placement_id: 0,
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                x_offset: 0,
                y_offset: 0,
                columns: 0,
                rows: 0,
                cursor_movement: CursorMovement::After,
                virtual_placement: false,
                parent_id: 0,
                parent_placement_id: 0,
                horizontal_offset: 0,
                vertical_offset: 0,
                z: 0,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=p,i=+12,z=+12",
        expected: Ok((
            Control::Display(Display {
                image_id: 12,
                image_number: 0,
                placement_id: 0,
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                x_offset: 0,
                y_offset: 0,
                columns: 0,
                rows: 0,
                cursor_movement: CursorMovement::After,
                virtual_placement: false,
                parent_id: 0,
                parent_placement_id: 0,
                horizontal_offset: 0,
                vertical_offset: 0,
                z: 12,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=p,i=1_2,z=-1_2",
        expected: Ok((
            Control::Display(Display {
                image_id: 12,
                image_number: 0,
                placement_id: 0,
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                x_offset: 0,
                y_offset: 0,
                columns: 0,
                rows: 0,
                cursor_movement: CursorMovement::After,
                virtual_placement: false,
                parent_id: 0,
                parent_placement_id: 0,
                horizontal_offset: 0,
                vertical_offset: 0,
                z: -12,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=p,i=-0",
        expected: Ok((
            Control::Display(Display {
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                x_offset: 0,
                y_offset: 0,
                columns: 0,
                rows: 0,
                cursor_movement: CursorMovement::After,
                virtual_placement: false,
                parent_id: 0,
                parent_placement_id: 0,
                horizontal_offset: 0,
                vertical_offset: 0,
                z: 0,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=p,i=1__2",
        expected: Ok((
            Control::Display(Display {
                image_id: 12,
                image_number: 0,
                placement_id: 0,
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                x_offset: 0,
                y_offset: 0,
                columns: 0,
                rows: 0,
                cursor_movement: CursorMovement::After,
                virtual_placement: false,
                parent_id: 0,
                parent_placement_id: 0,
                horizontal_offset: 0,
                vertical_offset: 0,
                z: 0,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=p,i=",
        expected: Err(ParseError::InvalidCharacter),
    },
    CommandCase {
        input: "a=p,i=12x",
        expected: Err(ParseError::InvalidCharacter),
    },
    CommandCase {
        input: "a=p,i=-12",
        expected: Err(ParseError::Overflow),
    },
    CommandCase {
        input: "a=p,i=_12",
        expected: Err(ParseError::InvalidCharacter),
    },
    CommandCase {
        input: "a=p,i=12_",
        expected: Err(ParseError::InvalidCharacter),
    },
    CommandCase {
        input: "a=p,i= 12",
        expected: Err(ParseError::InvalidCharacter),
    },
];

#[test]
fn duplicate_and_unknown_control_keys_keep_parser_state() {
    assert_cases(DUPLICATE_AND_UNKNOWN_CONTROL_KEYS_KEEP_PARSER_STATE_CASES);
}
const DUPLICATE_AND_UNKNOWN_CONTROL_KEYS_KEEP_PARSER_STATE_CASES: &[CommandCase] = &[
    CommandCase {
        input: "a=p,i=1,i=2",
        expected: Ok((
            Control::Display(Display {
                image_id: 2,
                image_number: 0,
                placement_id: 0,
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                x_offset: 0,
                y_offset: 0,
                columns: 0,
                rows: 0,
                cursor_movement: CursorMovement::After,
                virtual_placement: false,
                parent_id: 0,
                parent_placement_id: 0,
                horizontal_offset: 0,
                vertical_offset: 0,
                z: 0,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=p,long=value,i=7,z=-4",
        expected: Ok((
            Control::Display(Display {
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                x_offset: 0,
                y_offset: 0,
                columns: 0,
                rows: 0,
                cursor_movement: CursorMovement::After,
                virtual_placement: false,
                parent_id: 0,
                parent_placement_id: 0,
                horizontal_offset: 0,
                vertical_offset: 0,
                z: 0,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=p,e=77,i=7",
        expected: Ok((
            Control::Display(Display {
                image_id: 7,
                image_number: 0,
                placement_id: 0,
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                x_offset: 0,
                y_offset: 0,
                columns: 0,
                rows: 0,
                cursor_movement: CursorMovement::After,
                virtual_placement: false,
                parent_id: 0,
                parent_placement_id: 0,
                horizontal_offset: 0,
                vertical_offset: 0,
                z: 0,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=p,abcdefghijkl=value,i=7",
        expected: Ok((
            Control::Display(Display {
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                x_offset: 0,
                y_offset: 0,
                columns: 0,
                rows: 0,
                cursor_movement: CursorMovement::After,
                virtual_placement: false,
                parent_id: 0,
                parent_placement_id: 0,
                horizontal_offset: 0,
                vertical_offset: 0,
                z: 0,
            }),
            Quiet::No,
            &[],
        )),
    },
];

#[test]
fn unsigned_overflow_is_reported() {
    assert_cases(UNSIGNED_OVERFLOW_IS_REPORTED_CASES);
}
const UNSIGNED_OVERFLOW_IS_REPORTED_CASES: &[CommandCase] = &[CommandCase {
    input: "a=p,i=10000000000",
    expected: Err(ParseError::Overflow),
}];

#[test]
fn signed_overflow_is_reported() {
    assert_cases(SIGNED_OVERFLOW_IS_REPORTED_CASES);
}
const SIGNED_OVERFLOW_IS_REPORTED_CASES: &[CommandCase] = &[CommandCase {
    input: "a=p,z=-9999999999",
    expected: Err(ParseError::Overflow),
}];

#[test]
fn parser_completion_states_are_distinct() {
    assert!(matches!(
        Parser::parse_string(b"a=p,i=246").unwrap().control,
        Control::Display(_)
    ));
    assert_eq!(
        Parser::parse_string(b"i=000000000000")
            .unwrap()
            .transmission()
            .unwrap()
            .image_id,
        0
    );
    assert_cases(PARSER_COMPLETION_STATES_ARE_DISTINCT_CASES);
}
const PARSER_COMPLETION_STATES_ARE_DISTINCT_CASES: &[CommandCase] = &[
    CommandCase {
        input: "a=p,i",
        expected: Err(ParseError::InvalidFormat),
    },
    CommandCase {
        input: "a=p,",
        expected: Err(ParseError::InvalidFormat),
    },
    CommandCase {
        input: "",
        expected: Err(ParseError::InvalidFormat),
    },
    CommandCase {
        input: ";",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Rgba,
                medium: Medium::Direct,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: false,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=t;",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Rgba,
                medium: Medium::Direct,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: false,
            }),
            Quiet::No,
            &[],
        )),
    },
];

#[test]
fn transmission_keys_and_defaults_are_complete() {
    let cmd = Parser::parse_string(b"f=24,t=d,s=11,v=12,S=13,O=14,i=15,I=16,p=17,o=z,m=1").unwrap();
    assert_eq!(
        cmd.control,
        Control::Transmit(Transmission {
            format: Format::Rgb,
            medium: Medium::Direct,
            width: 11,
            height: 12,
            size: 13,
            offset: 14,
            image_id: 15,
            image_number: 16,
            placement_id: 17,
            compression: Compression::ZlibDeflate,
            more_chunks: true
        })
    );
    assert_cases(TRANSMISSION_KEYS_AND_DEFAULTS_ARE_COMPLETE_CASES);
}
const TRANSMISSION_KEYS_AND_DEFAULTS_ARE_COMPLETE_CASES: &[CommandCase] = &[
    CommandCase {
        input: "f=24;",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Rgb,
                medium: Medium::Direct,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: false,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "f=32;",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Rgba,
                medium: Medium::Direct,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: false,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "f=100;",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Png,
                medium: Medium::Direct,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: false,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "f=99;",
        expected: Err(ParseError::InvalidFormat),
    },
    CommandCase {
        input: "t=x;",
        expected: Err(ParseError::InvalidFormat),
    },
    CommandCase {
        input: "o=x;",
        expected: Err(ParseError::InvalidFormat),
    },
    CommandCase {
        input: "f= ;",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Rgba,
                medium: Medium::Direct,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: false,
            }),
            Quiet::No,
            &[],
        )),
    },
];

#[test]
fn display_keys_and_defaults_are_complete() {
    let cmd=Parser::parse_string(b"a=p,i=21,I=22,p=23,x=24,y=25,w=26,h=27,X=28,Y=29,c=30,r=31,C=1,U=1,P=32,Q=33,H=-34,V=-35,z=-36").unwrap();
    assert_eq!(
        cmd.control,
        Control::Display(Display {
            image_id: 21,
            image_number: 22,
            placement_id: 23,
            x: 24,
            y: 25,
            width: 26,
            height: 27,
            x_offset: 28,
            y_offset: 29,
            columns: 30,
            rows: 31,
            cursor_movement: CursorMovement::None,
            virtual_placement: true,
            parent_id: 32,
            parent_placement_id: 33,
            horizontal_offset: -34,
            vertical_offset: -35,
            z: -36
        })
    );
    assert_cases(DISPLAY_KEYS_AND_DEFAULTS_ARE_COMPLETE_CASES);
}
const DISPLAY_KEYS_AND_DEFAULTS_ARE_COMPLETE_CASES: &[CommandCase] = &[
    CommandCase {
        input: "a=p,C=2;",
        expected: Err(ParseError::InvalidFormat),
    },
    CommandCase {
        input: "a=p,U=2;",
        expected: Err(ParseError::InvalidFormat),
    },
];

#[test]
fn actions_and_quiet_levels_are_validated() {
    for (action, expected) in [
        ("q", Control::Query(Transmission::default())),
        ("t", Control::Transmit(Transmission::default())),
        (
            "T",
            Control::TransmitAndDisplay {
                transmission: Transmission::default(),
                display: Display::default(),
            },
        ),
        ("p", Control::Display(Display::default())),
        ("d", Control::Delete(Delete::All(false))),
        (
            "f",
            Control::TransmitAnimationFrame(AnimationFrameLoading::default()),
        ),
        ("a", Control::ControlAnimation(AnimationControl::default())),
        (
            "c",
            Control::ComposeAnimation(AnimationFrameComposition::default()),
        ),
    ] {
        let cmd = Parser::parse_string(format!("a={action},q=1;").as_bytes()).unwrap();
        assert_eq!(cmd.control, expected);
        assert_eq!(cmd.quiet, Quiet::Ok);
    }
    assert_cases(ACTIONS_AND_QUIET_LEVELS_ARE_VALIDATED_CASES);
}
const ACTIONS_AND_QUIET_LEVELS_ARE_VALIDATED_CASES: &[CommandCase] = &[
    CommandCase {
        input: "a=p,q=0",
        expected: Ok((
            Control::Display(Display {
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                x_offset: 0,
                y_offset: 0,
                columns: 0,
                rows: 0,
                cursor_movement: CursorMovement::After,
                virtual_placement: false,
                parent_id: 0,
                parent_placement_id: 0,
                horizontal_offset: 0,
                vertical_offset: 0,
                z: 0,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=p,q=1",
        expected: Ok((
            Control::Display(Display {
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                x_offset: 0,
                y_offset: 0,
                columns: 0,
                rows: 0,
                cursor_movement: CursorMovement::After,
                virtual_placement: false,
                parent_id: 0,
                parent_placement_id: 0,
                horizontal_offset: 0,
                vertical_offset: 0,
                z: 0,
            }),
            Quiet::Ok,
            &[],
        )),
    },
    CommandCase {
        input: "a=p,q=2",
        expected: Ok((
            Control::Display(Display {
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                x_offset: 0,
                y_offset: 0,
                columns: 0,
                rows: 0,
                cursor_movement: CursorMovement::After,
                virtual_placement: false,
                parent_id: 0,
                parent_placement_id: 0,
                horizontal_offset: 0,
                vertical_offset: 0,
                z: 0,
            }),
            Quiet::Failures,
            &[],
        )),
    },
    CommandCase {
        input: "a=p,q=3;",
        expected: Err(ParseError::InvalidFormat),
    },
    CommandCase {
        input: "a=x;",
        expected: Err(ParseError::InvalidFormat),
    },
    CommandCase {
        input: "a=256;",
        expected: Err(ParseError::InvalidFormat),
    },
];

#[test]
fn delete_selectors_retain_each_variant() {
    assert_cases(DELETE_SELECTORS_RETAIN_EACH_VARIANT_CASES);
}
const DELETE_SELECTORS_RETAIN_EACH_VARIANT_CASES: &[CommandCase] = &[
    CommandCase {
        input: "a=d;",
        expected: Ok((Control::Delete(Delete::All(false)), Quiet::No, &[])),
    },
    CommandCase {
        input: "a=d,d=r,x=3,y=3",
        expected: Ok((
            Control::Delete(Delete::Range {
                delete: false,
                first: 3,
                last: 3,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=d,d=b;",
        expected: Err(ParseError::InvalidFormat),
    },
    CommandCase {
        input: "a=d,d=a,i=11,I=12,p=13,x=3,y=4,z=-5",
        expected: Ok((Control::Delete(Delete::All(false)), Quiet::No, &[])),
    },
    CommandCase {
        input: "a=d,d=A,i=11,I=12,p=13,x=3,y=4,z=-5",
        expected: Ok((Control::Delete(Delete::All(true)), Quiet::No, &[])),
    },
    CommandCase {
        input: "a=d,d=i,i=11,I=12,p=13,x=3,y=4,z=-5",
        expected: Ok((
            Control::Delete(Delete::Id {
                delete: false,
                image_id: 11,
                placement_id: 13,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=d,d=I,i=11,I=12,p=13,x=3,y=4,z=-5",
        expected: Ok((
            Control::Delete(Delete::Id {
                delete: true,
                image_id: 11,
                placement_id: 13,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=d,d=n,i=11,I=12,p=13,x=3,y=4,z=-5",
        expected: Ok((
            Control::Delete(Delete::Newest {
                delete: false,
                image_number: 12,
                placement_id: 13,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=d,d=N,i=11,I=12,p=13,x=3,y=4,z=-5",
        expected: Ok((
            Control::Delete(Delete::Newest {
                delete: true,
                image_number: 12,
                placement_id: 13,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=d,d=c,i=11,I=12,p=13,x=3,y=4,z=-5",
        expected: Ok((
            Control::Delete(Delete::IntersectCursor(false)),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=d,d=C,i=11,I=12,p=13,x=3,y=4,z=-5",
        expected: Ok((
            Control::Delete(Delete::IntersectCursor(true)),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=d,d=f,i=11,I=12,p=13,x=3,y=4,z=-5",
        expected: Ok((
            Control::Delete(Delete::AnimationFrames(false)),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=d,d=F,i=11,I=12,p=13,x=3,y=4,z=-5",
        expected: Ok((
            Control::Delete(Delete::AnimationFrames(true)),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=d,d=p,i=11,I=12,p=13,x=3,y=4,z=-5",
        expected: Ok((
            Control::Delete(Delete::IntersectCell {
                delete: false,
                x: 3,
                y: 4,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=d,d=P,i=11,I=12,p=13,x=3,y=4,z=-5",
        expected: Ok((
            Control::Delete(Delete::IntersectCell {
                delete: true,
                x: 3,
                y: 4,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=d,d=q,i=11,I=12,p=13,x=3,y=4,z=-5",
        expected: Ok((
            Control::Delete(Delete::IntersectCellZ {
                delete: false,
                x: 3,
                y: 4,
                z: -5,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=d,d=Q,i=11,I=12,p=13,x=3,y=4,z=-5",
        expected: Ok((
            Control::Delete(Delete::IntersectCellZ {
                delete: true,
                x: 3,
                y: 4,
                z: -5,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=d,d=r,i=11,I=12,p=13,x=3,y=4,z=-5",
        expected: Ok((
            Control::Delete(Delete::Range {
                delete: false,
                first: 3,
                last: 4,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=d,d=R,i=11,I=12,p=13,x=3,y=4,z=-5",
        expected: Ok((
            Control::Delete(Delete::Range {
                delete: true,
                first: 3,
                last: 4,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=d,d=x,i=11,I=12,p=13,x=3,y=4,z=-5",
        expected: Ok((
            Control::Delete(Delete::Column {
                delete: false,
                x: 3,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=d,d=X,i=11,I=12,p=13,x=3,y=4,z=-5",
        expected: Ok((
            Control::Delete(Delete::Column { delete: true, x: 3 }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=d,d=y,i=11,I=12,p=13,x=3,y=4,z=-5",
        expected: Ok((
            Control::Delete(Delete::Row {
                delete: false,
                y: 4,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=d,d=Y,i=11,I=12,p=13,x=3,y=4,z=-5",
        expected: Ok((
            Control::Delete(Delete::Row { delete: true, y: 4 }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=d,d=z,i=11,I=12,p=13,x=3,y=4,z=-5",
        expected: Ok((
            Control::Delete(Delete::Z {
                delete: false,
                z: -5,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=d,d=Z,i=11,I=12,p=13,x=3,y=4,z=-5",
        expected: Ok((
            Control::Delete(Delete::Z {
                delete: true,
                z: -5,
            }),
            Quiet::No,
            &[],
        )),
    },
];

#[test]
fn reversed_range_is_rejected() {
    assert_cases(REVERSED_RANGE_IS_REJECTED_CASES);
}
const REVERSED_RANGE_IS_REJECTED_CASES: &[CommandCase] = &[CommandCase {
    input: "a=d,d=R,x=5,y=4",
    expected: Err(ParseError::InvalidFormat),
}];

#[test]
fn range_requires_its_last_endpoint() {
    assert_cases(RANGE_REQUIRES_ITS_LAST_ENDPOINT_CASES);
}
const RANGE_REQUIRES_ITS_LAST_ENDPOINT_CASES: &[CommandCase] = &[CommandCase {
    input: "a=d,d=R,x=5",
    expected: Err(ParseError::InvalidFormat),
}];

#[test]
fn range_requires_its_first_endpoint() {
    assert_cases(RANGE_REQUIRES_ITS_FIRST_ENDPOINT_CASES);
}
const RANGE_REQUIRES_ITS_FIRST_ENDPOINT_CASES: &[CommandCase] = &[CommandCase {
    input: "a=d,d=R,y=5",
    expected: Err(ParseError::InvalidFormat),
}];

#[test]
fn local_media_ignore_chunk_continuation() {
    assert_cases(LOCAL_MEDIA_IGNORE_CHUNK_CONTINUATION_CASES);
}
const LOCAL_MEDIA_IGNORE_CHUNK_CONTINUATION_CASES: &[CommandCase] = &[
    CommandCase {
        input: "a=t,t=f,m=0",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Rgba,
                medium: Medium::File,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: false,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=t,t=f,m=1",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Rgba,
                medium: Medium::File,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: false,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=t,t=f,m=2",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Rgba,
                medium: Medium::File,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: false,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=t,t=t,m=0",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Rgba,
                medium: Medium::TemporaryFile,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: false,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=t,t=t,m=1",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Rgba,
                medium: Medium::TemporaryFile,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: false,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=t,t=t,m=2",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Rgba,
                medium: Medium::TemporaryFile,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: false,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=t,t=s,m=0",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Rgba,
                medium: Medium::SharedMemory,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: false,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=t,t=s,m=1",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Rgba,
                medium: Medium::SharedMemory,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: false,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=t,t=s,m=2",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Rgba,
                medium: Medium::SharedMemory,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: false,
            }),
            Quiet::No,
            &[],
        )),
    },
];

#[test]
fn direct_media_retain_chunk_continuation() {
    assert_cases(DIRECT_MEDIA_RETAIN_CHUNK_CONTINUATION_CASES);
}
const DIRECT_MEDIA_RETAIN_CHUNK_CONTINUATION_CASES: &[CommandCase] = &[
    CommandCase {
        input: "a=t,t=d,m=0",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Rgba,
                medium: Medium::Direct,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: false,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=t,t=d,m=1",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Rgba,
                medium: Medium::Direct,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: true,
            }),
            Quiet::No,
            &[],
        )),
    },
    CommandCase {
        input: "a=t,t=d,m=2",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Rgba,
                medium: Medium::Direct,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: true,
            }),
            Quiet::No,
            &[],
        )),
    },
];

#[test]
fn payload_decoder_accepts_shipped_forms() {
    assert_cases(PAYLOAD_DECODER_ACCEPTS_SHIPPED_FORMS_CASES);
}
const PAYLOAD_DECODER_ACCEPTS_SHIPPED_FORMS_CASES: &[CommandCase] = &[
    CommandCase {
        input: "a=t;QQ",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Rgba,
                medium: Medium::Direct,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: false,
            }),
            Quiet::No,
            &[65],
        )),
    },
    CommandCase {
        input: "a=t;QQ==",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Rgba,
                medium: Medium::Direct,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: false,
            }),
            Quiet::No,
            &[65],
        )),
    },
    CommandCase {
        input: "a=t;QR==",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Rgba,
                medium: Medium::Direct,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: false,
            }),
            Quiet::No,
            &[65],
        )),
    },
    CommandCase {
        input: "a=t;Q Q==",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Rgba,
                medium: Medium::Direct,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: false,
            }),
            Quiet::No,
            &[65],
        )),
    },
    CommandCase {
        input: "a=t;Q\tQ==",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Rgba,
                medium: Medium::Direct,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: false,
            }),
            Quiet::No,
            &[65],
        )),
    },
    CommandCase {
        input: "a=t;Q\nQ==",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Rgba,
                medium: Medium::Direct,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: false,
            }),
            Quiet::No,
            &[65],
        )),
    },
    CommandCase {
        input: "a=t;Q\rQ==",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Rgba,
                medium: Medium::Direct,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: false,
            }),
            Quiet::No,
            &[65],
        )),
    },
    CommandCase {
        input: "a=t;Q\x0cQ==",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Rgba,
                medium: Medium::Direct,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: false,
            }),
            Quiet::No,
            &[65],
        )),
    },
    CommandCase {
        input: "a=t;QQ== ",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Rgba,
                medium: Medium::Direct,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: false,
            }),
            Quiet::No,
            &[65],
        )),
    },
    CommandCase {
        input: "a=t;QQ = =",
        expected: Ok((
            Control::Transmit(Transmission {
                format: Format::Rgba,
                medium: Medium::Direct,
                width: 0,
                height: 0,
                size: 0,
                offset: 0,
                image_id: 0,
                image_number: 0,
                placement_id: 0,
                compression: Compression::None,
                more_chunks: false,
            }),
            Quiet::No,
            &[65],
        )),
    },
];

#[test]
fn payload_decoder_rejects_other_forms() {
    assert_eq!(Parser::parse_string(b";\xff"), Err(ParseError::InvalidData));
    assert_cases(PAYLOAD_DECODER_REJECTS_OTHER_FORMS_CASES);
}
const PAYLOAD_DECODER_REJECTS_OTHER_FORMS_CASES: &[CommandCase] = &[
    CommandCase {
        input: "a=t;QQ=",
        expected: Err(ParseError::InvalidData),
    },
    CommandCase {
        input: "a=t;QQ===",
        expected: Err(ParseError::InvalidData),
    },
    CommandCase {
        input: "a=t;QQ====",
        expected: Err(ParseError::InvalidData),
    },
    CommandCase {
        input: "a=t;Q",
        expected: Err(ParseError::InvalidData),
    },
    CommandCase {
        input: "a=t;=",
        expected: Err(ParseError::InvalidData),
    },
    CommandCase {
        input: "a=t;====",
        expected: Err(ParseError::InvalidData),
    },
    CommandCase {
        input: "a=t;Q\x0bQ==",
        expected: Err(ParseError::InvalidData),
    },
    CommandCase {
        input: "a=t;Q-Q=",
        expected: Err(ParseError::InvalidData),
    },
    CommandCase {
        input: "a=t;Q_Q=",
        expected: Err(ParseError::InvalidData),
    },
    CommandCase {
        input: "a=t;QQ==AA",
        expected: Err(ParseError::InvalidData),
    },
    CommandCase {
        input: "a=t;AA AA",
        expected: Err(ParseError::InvalidData),
    },
    CommandCase {
        input: "a=t;AAAA",
        expected: Err(ParseError::InvalidData),
    },
];

/// Writer returning its original owned error on the first write.
struct FailingWriter(Option<std::io::Error>);
impl std::io::Write for FailingWriter {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(self.0.take().unwrap())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
