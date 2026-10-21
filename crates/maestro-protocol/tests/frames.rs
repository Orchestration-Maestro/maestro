//! Binary frame and version negotiation observations.
//! Adapted for Maestro: names, module layout and Rust documentation changed.
#![cfg(test)]

use maestro_protocol::frames::*;
use std::io::{self, Read};

// ---- Round-trip: ClientMessage ----

#[test]
/// Client hello roundtrip.
fn handshake_fields_survive_binary_encoding() {
    let msg = hello_message();
    let encoded = bincode::serde::encode_to_vec(&msg, bincode::config::standard()).unwrap();
    let (decoded, _): (ClientMessage, _) =
        bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
/// Client input roundtrip.
fn input_bytes_survive_binary_encoding() {
    let msg = ClientMessage::Input {
        data: vec![0x1b, 0x5b, 0x41], // ESC [ A (up arrow)
    };
    let encoded = bincode::serde::encode_to_vec(&msg, bincode::config::standard()).unwrap();
    let (decoded, _): (ClientMessage, _) =
        bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
/// Client message wire tags preserve protocol 15 order.
fn client_variant_discriminants_keep_their_wire_values() {
    /// Tag.
    fn tag(msg: &ClientMessage) -> u8 {
        *bincode::serde::encode_to_vec(msg, bincode::config::standard())
            .unwrap()
            .first()
            .expect("encoded client message should include enum tag")
    }

    assert_eq!(tag(&hello_message()), 0);
    assert_eq!(tag(&ClientMessage::Input { data: Vec::new() }), 1);
    assert_eq!(
        tag(&ClientMessage::ClipboardImage {
            extension: "png".to_owned(),
            data: Vec::new(),
        }),
        2
    );
    assert_eq!(
        tag(&ClientMessage::Resize {
            cols: 80,
            rows: 24,
            cell_width_px: 8,
            cell_height_px: 16,
        }),
        3
    );
    assert_eq!(tag(&ClientMessage::Detach), 4);
    assert_eq!(
        tag(&ClientMessage::AttachTerminal {
            terminal_id: "term".to_owned(),
            takeover: false,
        }),
        5
    );
    assert_eq!(
        tag(&ClientMessage::AttachScroll {
            source: AttachScrollSource::Wheel,
            direction: AttachScrollDirection::Up,
            lines: 1,
            column: None,
            row: None,
            modifiers: 0,
        }),
        6
    );
    assert_eq!(tag(&ClientMessage::InputEvents { events: Vec::new() }), 7);
    assert_eq!(
        tag(&ClientMessage::ObserveTerminal {
            target: "w1:p1".to_owned(),
        }),
        8
    );
    assert_eq!(
        tag(&ClientMessage::ControlTerminal {
            target: "w1:p1".to_owned(),
            takeover: false,
        }),
        9
    );
}

#[test]
/// Client input events roundtrip.
fn structured_input_events_survive_binary_encoding() {
    let msg = ClientMessage::InputEvents {
        events: vec![
            ClientInputEvent::Key {
                code: ClientKeyCode::Char('N'),
                modifiers: 1,
                kind: ClientKeyKind::Press,

                repeat_count: 1,
                generated_text: None,
                source: maestro_protocol::frames::ClientKeySource::Synthesized,
            },
            ClientInputEvent::Key {
                code: ClientKeyCode::Backspace,
                modifiers: 0,
                kind: ClientKeyKind::Press,
                repeat_count: 3,
                generated_text: None,
                source: maestro_protocol::frames::ClientKeySource::Vt {
                    bytes: b"\x1b[127;1u".to_vec(),
                },
            },
            ClientInputEvent::Key {
                code: ClientKeyCode::Esc,
                modifiers: 0,
                kind: ClientKeyKind::Release,
                repeat_count: 1,
                generated_text: None,
                source: maestro_protocol::frames::ClientKeySource::WindowsConsole {
                    record: maestro_protocol::shared::WindowsKeyRecord {
                        key_down: false,
                        repeat_count: 1,
                        virtual_key_code: 27,
                        virtual_scan_code: 1,
                        unicode: 27,
                        control_key_state: 0,
                    },
                },
            },
            ClientInputEvent::TextCommit("你🙂".to_owned()),
            ClientInputEvent::Mouse {
                kind: ClientMouseKind::Down(ClientMouseButton::Left),
                column: 3,
                row: 4,
                modifiers: 0,
            },
        ],
    };
    let encoded = bincode::serde::encode_to_vec(&msg, bincode::config::standard()).unwrap();
    // Freeze the protocol 20 input envelope before it is published.
    assert_eq!(
        encoded,
        vec![
            7, 5, 0, 15, 78, 1, 0, 1, 0, 0, 0, 0, 0, 0, 3, 0, 1, 8, 27, 91, 49, 50, 55, 59, 49,
            117, 0, 14, 0, 2, 1, 0, 2, 0, 1, 27, 1, 27, 0, 1, 7, 228, 189, 160, 240, 159, 153, 130,
            2, 0, 0, 3, 4, 0,
        ]
    );
    let (decoded, _): (ClientMessage, _) =
        bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
/// Client clipboard image roundtrip.
fn clipboard_image_bytes_survive_binary_encoding() {
    let msg = ClientMessage::ClipboardImage {
        extension: "png".to_owned(),
        data: vec![0x89, b'P', b'N', b'G'],
    };
    let encoded = bincode::serde::encode_to_vec(&msg, bincode::config::standard()).unwrap();
    let (decoded, _): (ClientMessage, _) =
        bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
/// Client input large multilingual payload roundtrip.
fn large_unicode_input_survives_binary_encoding() {
    let text = "你好，今天我们测试一段比较长的语音输入。こんにちは。안녕하세요.🙂".repeat(1024);
    assert!(text.len() > 64 * 1024);
    assert!(text.len() < MAX_FRAME_SIZE);
    let msg = ClientMessage::Input {
        data: text.as_bytes().to_vec(),
    };

    let encoded = bincode::serde::encode_to_vec(&msg, bincode::config::standard()).unwrap();
    let (decoded, consumed): (ClientMessage, _) =
        bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();

    assert_eq!(consumed, encoded.len());
    assert_eq!(decoded, msg);
}

#[test]
/// Client resize roundtrip.
fn terminal_dimensions_survive_binary_encoding() {
    let msg = ClientMessage::Resize {
        cols: 80,
        rows: 24,
        cell_width_px: 8,
        cell_height_px: 16,
    };
    let encoded = bincode::serde::encode_to_vec(&msg, bincode::config::standard()).unwrap();
    let (decoded, _): (ClientMessage, _) =
        bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
/// Client detach roundtrip.
fn detach_variant_survives_binary_encoding() {
    let msg = ClientMessage::Detach;
    let encoded = bincode::serde::encode_to_vec(&msg, bincode::config::standard()).unwrap();
    let (decoded, _): (ClientMessage, _) =
        bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
/// Client attach terminal roundtrip.
fn terminal_attachment_survives_binary_encoding() {
    let msg = ClientMessage::AttachTerminal {
        terminal_id: "term_123".to_owned(),
        takeover: true,
    };
    let encoded = bincode::serde::encode_to_vec(&msg, bincode::config::standard()).unwrap();
    let (decoded, _): (ClientMessage, _) =
        bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
/// Client observe terminal roundtrip.
fn terminal_observation_survives_binary_encoding() {
    let msg = ClientMessage::ObserveTerminal {
        target: "w1:p1".to_owned(),
    };
    let encoded = bincode::serde::encode_to_vec(&msg, bincode::config::standard()).unwrap();
    let (decoded, _): (ClientMessage, _) =
        bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
/// Client control terminal roundtrip.
fn terminal_control_survives_binary_encoding() {
    let msg = ClientMessage::ControlTerminal {
        target: "w1:p1".to_owned(),
        takeover: true,
    };
    let encoded = bincode::serde::encode_to_vec(&msg, bincode::config::standard()).unwrap();
    let (decoded, _): (ClientMessage, _) =
        bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
/// Client attach scroll roundtrip.
fn scroll_attachment_survives_binary_encoding() {
    let msg = ClientMessage::AttachScroll {
        source: AttachScrollSource::Wheel,
        direction: AttachScrollDirection::Up,
        lines: 3,
        column: Some(12),
        row: Some(7),
        modifiers: 4,
    };
    let encoded = bincode::serde::encode_to_vec(&msg, bincode::config::standard()).unwrap();
    let (decoded, _): (ClientMessage, _) =
        bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
    assert_eq!(msg, decoded);
}

// ---- Round-trip: ServerMessage ----

#[test]
/// Server welcome roundtrip.
fn welcome_fields_survive_binary_encoding() {
    let msg = ServerMessage::Welcome {
        version: PROTOCOL_VERSION,
        encoding: RenderEncoding::SemanticFrame,
        error: None,
    };
    let encoded = bincode::serde::encode_to_vec(&msg, bincode::config::standard()).unwrap();
    let (decoded, _): (ServerMessage, _) =
        bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
/// Server welcome with error roundtrip.
fn welcome_failure_survives_binary_encoding() {
    let msg = ServerMessage::Welcome {
        version: PROTOCOL_VERSION,
        encoding: RenderEncoding::SemanticFrame,
        error: Some("incompatible version".to_owned()),
    };
    let encoded = bincode::serde::encode_to_vec(&msg, bincode::config::standard()).unwrap();
    let (decoded, _): (ServerMessage, _) =
        bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
/// Server frame roundtrip nontrivial.
fn styled_screen_cells_survive_binary_encoding() {
    // Build a 3×2 frame with varied styles (≥2×2).
    let frame = FrameData {
        cells: varied_cells(),
        width: 3,
        height: 2,
        cursor: Some(CursorState {
            x: 0,
            y: 0,
            visible: true,
            shape: 6,
        }),
        hyperlinks: vec!["https://example.com".to_owned()],
        graphics: Vec::new(),
    };
    let msg = ServerMessage::Frame(frame);
    let encoded = bincode::serde::encode_to_vec(&msg, bincode::config::standard()).unwrap();
    let (decoded, _): (ServerMessage, _) =
        bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
    assert_eq!(msg, decoded);
    match decoded {
        ServerMessage::Frame(frame) => {
            assert_eq!(frame.cells[2].hyperlink, Some(0));
            assert_eq!(frame.hyperlinks, vec!["https://example.com".to_owned()]);
        }
        other => panic!("expected frame, got {other:?}"),
    }
}

#[test]
/// Server shutdown roundtrip.
fn shutdown_reason_survives_binary_encoding() {
    let msg = ServerMessage::ServerShutdown {
        reason: Some("updating".to_owned()),
    };
    let encoded = bincode::serde::encode_to_vec(&msg, bincode::config::standard()).unwrap();
    let (decoded, _): (ServerMessage, _) =
        bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
/// Server notify roundtrip.
fn notification_fields_survive_binary_encoding() {
    for kind in [
        NotifyKind::Sound,
        NotifyKind::Toast,
        NotifyKind::SystemToast,
    ] {
        let msg = ServerMessage::Notify {
            kind,
            message: "agent done".to_owned(),
            body: None,
        };
        let encoded = bincode::serde::encode_to_vec(&msg, bincode::config::standard()).unwrap();
        let (decoded, _): (ServerMessage, _) =
            bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
        assert_eq!(msg, decoded);
    }
}

#[test]
/// Server clipboard roundtrip.
fn clipboard_text_survives_binary_encoding() {
    let msg = ServerMessage::Clipboard {
        data: "dGVzdA==".to_owned(), // base64 "test"
    };
    let encoded = bincode::serde::encode_to_vec(&msg, bincode::config::standard()).unwrap();
    let (decoded, _): (ServerMessage, _) =
        bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
/// Server window title roundtrip.
fn window_title_survives_binary_encoding() {
    for title in [Some("maestro api".to_owned()), None] {
        let msg = ServerMessage::WindowTitle { title };
        let encoded = bincode::serde::encode_to_vec(&msg, bincode::config::standard()).unwrap();
        let (decoded, _): (ServerMessage, _) =
            bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
        assert_eq!(msg, decoded);
    }
}

#[test]
/// Server graphics roundtrip.
fn graphics_payload_survives_binary_encoding() {
    let msg = ServerMessage::Graphics {
        bytes: b"\x1b_Ga=d,d=A,q=2;\x1b\\".to_vec(),
    };
    let encoded = bincode::serde::encode_to_vec(&msg, bincode::config::standard()).unwrap();
    let (decoded, _): (ServerMessage, _) =
        bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
/// Server terminal frame roundtrip.
fn terminal_frame_bytes_survive_binary_encoding() {
    let msg = ServerMessage::Terminal(TerminalFrame {
        seq: 7,
        width: 120,
        height: 40,
        full: false,
        bytes: b"\x1b[1;1Hhello".to_vec(),
    });
    let encoded = bincode::serde::encode_to_vec(&msg, bincode::config::standard()).unwrap();
    let (decoded, _): (ServerMessage, _) =
        bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
/// Server reload sound config roundtrip.
fn sound_reload_variant_survives_binary_encoding() {
    let msg = ServerMessage::ReloadSoundConfig;
    let encoded = bincode::serde::encode_to_vec(&msg, bincode::config::standard()).unwrap();
    let (decoded, _): (ServerMessage, _) =
        bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
/// Server mouse capture roundtrip.
fn mouse_capture_setting_survives_binary_encoding() {
    let msg = ServerMessage::MouseCapture {
        enabled: true,
        sgr_pixels: true,
    };
    let encoded = bincode::serde::encode_to_vec(&msg, bincode::config::standard()).unwrap();
    let (decoded, _): (ServerMessage, _) =
        bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
/// Server kitty keyboard report all roundtrip.
fn keyboard_report_setting_survives_binary_encoding() {
    let msg = ServerMessage::KittyKeyboardReportAll { enabled: true };
    let encoded = bincode::serde::encode_to_vec(&msg, bincode::config::standard()).unwrap();
    let (decoded, _): (ServerMessage, _) =
        bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
/// Direct graphics messages roundtrip.
fn direct_graphics_commands_survive_binary_encoding() {
    let client = ClientMessage::GraphicsTransmissionResult {
        transfer_id: 7,
        image_id: 42,
        success: false,
    };
    let encoded = bincode::serde::encode_to_vec(&client, bincode::config::standard()).unwrap();
    let (decoded, _): (ClientMessage, _) =
        bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
    assert_eq!(client, decoded);

    let server = ServerMessage::GraphicsFile {
        path: "/run/user/1000/maestro/source/frame".into(),
        expected_len: 4,
        image_id: 42,
        transfer_id: 7,
        leading: b"\x1b[2;3H".to_vec(),
        control: "a=T,f=32,i=42,q=0".into(),
    };
    let encoded = bincode::serde::encode_to_vec(&server, bincode::config::standard()).unwrap();
    let (decoded, _): (ServerMessage, _) =
        bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
    assert_eq!(server, decoded);

    let pixels = ClientMessage::InputPixels {
        data: b"\x1b[<35;321;241M".to_vec(),
        cols: 80,
        rows: 24,
        width_px: 800,
        height_px: 480,
    };
    let encoded = bincode::serde::encode_to_vec(&pixels, bincode::config::standard()).unwrap();
    let (decoded, _): (ClientMessage, _) =
        bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
    assert_eq!(pixels, decoded);
}

#[test]
/// Server prefix input source roundtrip.
fn prefix_input_origin_survives_binary_encoding() {
    for active in [true, false] {
        let msg = ServerMessage::PrefixInputSource { active };
        let encoded = bincode::serde::encode_to_vec(&msg, bincode::config::standard()).unwrap();
        let (decoded, _): (ServerMessage, _) =
            bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
        assert_eq!(msg, decoded);
    }
}

#[test]
/// Server terminal bell roundtrip.
fn bell_count_survives_binary_encoding() {
    let msg = ServerMessage::TerminalBell { count: 3 };
    let encoded = bincode::serde::encode_to_vec(&msg, bincode::config::standard()).unwrap();
    let (decoded, _): (ServerMessage, _) =
        bincode::serde::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
    assert_eq!(msg, decoded);
}

// ---- Framing ----

#[test]
/// Framing small message roundtrip.
fn framed_handshake_preserves_client_fields() {
    let msg = ClientMessage::Hello {
        version: PROTOCOL_VERSION,
        cols: 80,
        rows: 24,
        cell_width_px: 8,
        cell_height_px: 16,
        requested_encoding: RenderEncoding::SemanticFrame,
        keybindings: ClientKeybindings::Server,
        launch_mode: ClientLaunchMode::App,
    };
    let mut buf = Vec::new();
    write_message(&mut buf, &msg).unwrap();
    let decoded: ClientMessage = read_message(&mut buf.as_slice(), MAX_FRAME_SIZE).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
/// Framing large payload roundtrip.
fn framed_screen_preserves_a_large_cell_payload() {
    // Create a Frame message that is ≥128 KB.
    // Use a large frame with verbose cell data to exceed 128 KB after bincode encoding.
    // 200×50 = 10000 cells. With varied symbols and styles, this should easily exceed 128 KB.
    let width: u16 = 200;
    let height: u16 = 50;
    let cells: Vec<CellData> = (0..(width as usize) * (height as usize))
        .map(|i| CellData {
            symbol: if i % 256 < 32 {
                " ".to_owned()
            } else {
                format!("{:03}", i % 1000)
            },
            fg: 0x02_00_00_80
                | (u32::try_from(i % 256).unwrap() << 16)
                | (u32::try_from((i / 256) % 256).unwrap() << 8),
            bg: 0x01_00_00_00 | u32::try_from(i % 256).unwrap(),
            modifier: u16::try_from(i % 16).unwrap(),
            skip: i % 100 == 0,
            hyperlink: None,
        })
        .collect();

    let frame = FrameData {
        cells,
        width,
        height,
        cursor: Some(CursorState {
            x: 10,
            y: 5,
            visible: true,
            shape: 0,
        }),
        hyperlinks: Vec::new(),
        graphics: Vec::new(),
    };
    let msg = ServerMessage::Frame(frame);

    let mut buf = Vec::new();
    write_message(&mut buf, &msg).unwrap();
    // Verify the payload is at least 128 KB
    assert!(
        buf.len() >= 128 * 1024,
        "framed payload should be >= 128 KB, got {} bytes",
        buf.len()
    );

    let decoded: ServerMessage = read_message(&mut buf.as_slice(), MAX_FRAME_SIZE).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
/// Framing multiple messages sequential.
fn consecutive_frames_decode_in_written_order() {
    // Write 100+ messages of varying types and read them back.
    let mut buf = Vec::new();
    let mut expected = Vec::new();

    for i in 0..150u32 {
        let msg = match i % 5 {
            0 => ClientMessage::Hello {
                version: PROTOCOL_VERSION,
                cols: (80 + (i % 40) as u16),
                rows: (24 + (i % 20) as u16),
                cell_width_px: 8,
                cell_height_px: 16,
                requested_encoding: RenderEncoding::SemanticFrame,
                keybindings: ClientKeybindings::Server,
                launch_mode: ClientLaunchMode::App,
            },
            1 => ClientMessage::Input {
                data: vec![u8::try_from(i % 256).unwrap(); (i as usize % 50) + 1],
            },
            2 => ClientMessage::ClipboardImage {
                extension: "png".to_owned(),
                data: vec![0x89, b'P', b'N', b'G', u8::try_from(i % 256).unwrap()],
            },
            3 => ClientMessage::Resize {
                cols: (100 + (i % 30) as u16),
                rows: (30 + (i % 10) as u16),
                cell_width_px: 8,
                cell_height_px: 16,
            },
            4 => ClientMessage::Detach,
            _ => unreachable!(),
        };
        write_message(&mut buf, &msg).unwrap();
        expected.push(msg);
    }

    let mut cursor = buf.as_slice();
    for expected_msg in &expected {
        let decoded: ClientMessage = read_message(&mut cursor, MAX_FRAME_SIZE).unwrap();
        assert_eq!(*expected_msg, decoded);
    }
}

#[test]
/// Framing oversized rejected without panic.
fn frame_length_limit_rejects_an_oversized_prefix() {
    // Craft a frame with a huge length prefix (4 GB claim).
    let mut buf: Vec<u8> = (u32::MAX).to_le_bytes().to_vec();
    // Add a few garbage bytes after the length prefix.
    buf.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);

    let result: Result<ClientMessage, FramingError> =
        read_message(&mut buf.as_slice(), MAX_FRAME_SIZE);
    match result {
        Err(FramingError::Oversized { claimed, max }) => {
            assert_eq!(claimed, u32::MAX as usize);
            assert_eq!(max, MAX_FRAME_SIZE);
        }
        other => panic!("expected Oversized error, got: {other:?}"),
    }
}

#[test]
/// Framing malformed payload rejected without panic.
fn invalid_payload_returns_a_framing_error() {
    // Valid length prefix pointing to garbage data.
    let payload = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02];
    let mut buf = u32::try_from(payload.len()).unwrap().to_le_bytes().to_vec();
    buf.extend_from_slice(&payload);

    let result: Result<ClientMessage, FramingError> =
        read_message(&mut buf.as_slice(), MAX_FRAME_SIZE);
    assert!(result.is_err(), "malformed payload should be rejected");
    match result {
        Err(FramingError::Bincode(_)) => {} // expected
        other => panic!("expected Bincode error, got: {other:?}"),
    }
}

#[test]
/// Framing truncated stream returns unexpected eof.
fn incomplete_frame_reports_unexpected_eof() {
    // Write a length prefix claiming 100 bytes, but only provide 4.
    let mut buf: Vec<u8> = 100u32.to_le_bytes().to_vec();
    buf.extend_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD]);

    let result: Result<ClientMessage, FramingError> =
        read_message(&mut buf.as_slice(), MAX_FRAME_SIZE);
    match result {
        Err(FramingError::UnexpectedEof) => {}
        other => panic!("expected UnexpectedEof, got: {other:?}"),
    }
}

#[test]
/// A client string length the frame cannot hold fails decoding.
fn client_string_length_beyond_the_frame_is_rejected() {
    // `AttachTerminal` whose `terminal_id` declares 0x2600_0000_0100_0000 bytes.
    let frame = [0x0a, 0, 0, 0, 0x05, 0xfd, 0, 0, 0, 0x01, 0, 0, 0, 0x26];
    let result: Result<ClientMessage, FramingError> =
        read_message(&mut frame.as_slice(), MAX_FRAME_SIZE);
    assert!(
        matches!(result, Err(FramingError::Bincode(_))),
        "{result:?}"
    );
}

#[test]
/// A server string length the frame cannot hold fails decoding.
fn server_string_length_beyond_the_frame_is_rejected() {
    // `ServerShutdown` whose `reason` declares 0x2600_0000_0100_0000 bytes.
    let frame = [
        0x0b, 0, 0, 0, 0x04, 0x01, 0xfd, 0, 0, 0, 0x01, 0, 0, 0, 0x26,
    ];
    let result: Result<ServerMessage, FramingError> =
        read_message(&mut frame.as_slice(), MAX_GRAPHICS_FRAME_SIZE);
    assert!(
        matches!(result, Err(FramingError::Bincode(_))),
        "{result:?}"
    );
}

#[test]
/// A string past the frame's cap fails on the decoding limit, not on missing bytes.
fn string_length_past_the_frame_cap_fails_on_the_limit() {
    // `AttachTerminal` whose `terminal_id` declares one byte more than the cap.
    let mut payload = vec![0x05, 0xfc];
    payload.extend_from_slice(&u32::try_from(MAX_FRAME_SIZE + 1).unwrap().to_le_bytes());
    let mut frame = u32::try_from(payload.len()).unwrap().to_le_bytes().to_vec();
    frame.extend_from_slice(&payload);

    let result: Result<ClientMessage, FramingError> =
        read_message(&mut frame.as_slice(), MAX_FRAME_SIZE);
    match result {
        Err(FramingError::Bincode(message)) => {
            assert!(message.contains("LimitExceeded"), "{message}");
        }
        other => panic!("expected the decoding limit, got: {other:?}"),
    }
}

#[test]
/// Strings past the normal and graphics caps round-trip under a cap that admits them.
fn strings_past_the_frame_caps_round_trip_under_a_larger_cap() {
    for (len, cap) in [
        (MAX_FRAME_SIZE + 1, MAX_GRAPHICS_FRAME_SIZE),
        (MAX_GRAPHICS_FRAME_SIZE + 1, 2 * MAX_GRAPHICS_FRAME_SIZE),
    ] {
        let msg = ServerMessage::Clipboard {
            data: "A".repeat(len),
        };
        let mut buf = Vec::new();
        write_message(&mut buf, &msg).unwrap();
        let decoded: ServerMessage = read_message(&mut buf.as_slice(), cap).unwrap();
        assert_eq!(decoded, msg);
    }
}

#[test]
/// Small frames cannot reserve strings sized for the protocol-wide cap.
fn string_allocation_limit_tracks_the_received_payload() {
    for tag in [vec![5], vec![4, 1]] {
        let mut payload = tag;
        payload.extend_from_slice(&[251, 0, 16]);
        let mut frame = u32::try_from(payload.len()).unwrap().to_le_bytes().to_vec();
        frame.extend_from_slice(&payload);
        let error = if payload[0] == 5 {
            read_message::<_, ClientMessage>(&mut frame.as_slice(), MAX_FRAME_SIZE).unwrap_err()
        } else {
            read_message::<_, ServerMessage>(&mut frame.as_slice(), MAX_FRAME_SIZE).unwrap_err()
        };
        assert!(
            matches!(error, FramingError::Bincode(ref message) if message.contains("LimitExceeded")),
            "{error:?}"
        );
    }
}

#[test]
/// Admitted clipboard strings retain their cap-edge round trips.
fn cap_edge_clipboard_round_trips() {
    for cap in [MAX_FRAME_SIZE, MAX_GRAPHICS_FRAME_SIZE] {
        assert_cap_round_trip(
            &ServerMessage::Clipboard {
                data: "A".repeat(cap - 8),
            },
            cap,
            cap - 2,
        );
    }
}

#[test]
/// Admitted client byte vectors retain their cap-edge round trips.
fn cap_edge_client_bytes_round_trip() {
    for cap in [MAX_FRAME_SIZE, MAX_GRAPHICS_FRAME_SIZE] {
        assert_cap_round_trip(
            &ClientMessage::Input {
                data: vec![0; cap - 6],
            },
            cap,
            cap,
        );
    }
}

#[test]
/// Admitted graphics bytes retain their cap-edge round trips.
fn cap_edge_graphics_round_trip() {
    for cap in [MAX_FRAME_SIZE, MAX_GRAPHICS_FRAME_SIZE] {
        assert_cap_round_trip(
            &ServerMessage::Graphics {
                bytes: vec![0; cap - 6],
            },
            cap,
            cap,
        );
    }
}

/// Checks both wire size and decoding through the public frame reader.
fn assert_cap_round_trip<M>(message: &M, cap: usize, payload_len: usize)
where
    M: serde::Serialize + for<'de> serde::Deserialize<'de> + PartialEq + std::fmt::Debug,
{
    let mut buf = Vec::new();
    write_message(&mut buf, message).unwrap();
    assert_eq!(buf.len() - 4, payload_len);
    assert_eq!(
        &read_message::<_, M>(&mut buf.as_slice(), cap).unwrap(),
        message
    );
}

#[test]
/// Compact cell collections decode even when accounting exceeds their wire size.
fn dense_cells_at_the_frame_cap_round_trip() {
    let cell = CellData {
        symbol: String::new(),
        fg: 0,
        bg: 0,
        modifier: 0,
        skip: false,
        hyperlink: None,
    };
    let count = (MAX_FRAME_SIZE - 11) / 6;
    let msg = ServerMessage::Frame(FrameData {
        cells: vec![cell; count],
        width: 1,
        height: 1,
        cursor: None,
        hyperlinks: Vec::new(),
        graphics: vec![0; 3],
    });
    let mut buf = Vec::new();
    write_message(&mut buf, &msg).unwrap();
    assert_eq!(buf.len() - 4, MAX_FRAME_SIZE);
    assert_eq!(
        read_message::<_, ServerMessage>(&mut buf.as_slice(), MAX_FRAME_SIZE).unwrap(),
        msg
    );
}

#[test]
/// The detach frame prefix records the payload length.
fn detach_frame_prefix_matches_its_payload_length() {
    // Detach is a one-byte bincode payload.
    let msg = ClientMessage::Detach;
    let mut buf = Vec::new();
    write_message(&mut buf, &msg).unwrap();

    // Verify the length prefix is correct
    let len = u32::from_le_bytes(buf[..4].try_into().unwrap()) as usize;
    assert_eq!(
        len,
        buf.len() - 4,
        "length prefix should match payload size"
    );

    let decoded: ClientMessage = read_message(&mut buf.as_slice(), MAX_FRAME_SIZE).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
/// Framing partial read reassembly.
fn fragmented_reads_reassemble_the_input_payload() {
    // Simulate partial reads by using a reader that yields small chunks.
    let msg = ClientMessage::Input {
        data: vec![42; 500], // 500-byte input payload
    };
    let mut full_buf = Vec::new();
    write_message(&mut full_buf, &msg).unwrap();

    // Wrap in a chunked reader that only yields 7 bytes at a time.
    let mut chunked = ChunkedReader::new(full_buf, 7);
    let decoded: ClientMessage = read_message(&mut chunked, MAX_FRAME_SIZE).unwrap();
    assert_eq!(msg, decoded);
}

// ---- Version negotiation ----

#[test]
/// Version compatible.
fn equal_protocol_versions_allow_connection() {
    assert_eq!(
        check_client_version(PROTOCOL_VERSION),
        VersionCheck::Compatible
    );
}

#[test]
/// Version older client rejected.
fn older_protocol_version_requires_a_client_upgrade() {
    let result = check_client_version(PROTOCOL_VERSION - 1);
    assert!(matches!(result, VersionCheck::Incompatible(_)));
    if let VersionCheck::Incompatible(msg) = result {
        assert!(msg.contains("older"), "error should mention older version");
    }
}

#[test]
/// Version newer client rejected.
fn newer_protocol_version_requires_a_server_upgrade() {
    let result = check_client_version(PROTOCOL_VERSION + 1);
    assert!(matches!(result, VersionCheck::Incompatible(_)));
    if let VersionCheck::Incompatible(msg) = result {
        assert!(msg.contains("newer"), "error should mention newer version");
    }
}

// ---- Pre-persistence client rejection ----

#[test]
/// Prepersistence version zero rejected.
fn zero_protocol_version_cannot_connect() {
    let result = check_client_version(0);
    match result {
        VersionCheck::Incompatible(msg) => {
            assert!(
                msg.contains("pre-persistence"),
                "error should mention pre-persistence: {msg}"
            );
        }
        VersionCheck::Compatible => panic!("version 0 should be rejected as incompatible"),
    }
}

#[test]
/// Prepersistence version zero welcome has error.
fn zero_version_failure_survives_the_welcome_record() {
    // Simulating what the server would send to a v0 client.
    let check = check_client_version(0);
    let response = match check {
        VersionCheck::Compatible => ServerMessage::Welcome {
            version: PROTOCOL_VERSION,
            encoding: RenderEncoding::SemanticFrame,
            error: None,
        },
        VersionCheck::Incompatible(reason) => ServerMessage::Welcome {
            version: PROTOCOL_VERSION,
            encoding: RenderEncoding::SemanticFrame,
            error: Some(reason),
        },
    };

    match response {
        ServerMessage::Welcome { error: Some(_), .. } => {}
        other => panic!("expected Welcome with error, got: {other:?}"),
    }
}

// ---- Malformed/oversized input ----

#[test]
/// Oversized frame does not panic.
fn maximal_length_prefix_returns_an_error() {
    // Claim 4GB payload — should return Oversized error, not panic.
    let mut buf: Vec<u8> = 0xFFC0_0000_u32.to_le_bytes().to_vec(); // ~4 GB claim
    buf.extend_from_slice(&[0; 8]);

    let result: Result<ClientMessage, FramingError> =
        read_message(&mut buf.as_slice(), MAX_FRAME_SIZE);
    assert!(result.is_err());
    // Did not panic — test passing is proof.
}

#[test]
/// Malformed frame does not panic.
fn invalid_binary_discriminant_returns_an_error() {
    // Random garbage bytes after a valid-ish length prefix.
    let garbage: Vec<u8> = (0_u8..200).map(|i| i ^ 0xAA).collect();
    let mut buf = u32::try_from(garbage.len()).unwrap().to_le_bytes().to_vec();
    buf.extend_from_slice(&garbage);

    let result: Result<ClientMessage, FramingError> =
        read_message(&mut buf.as_slice(), MAX_FRAME_SIZE);
    assert!(result.is_err());
    // Did not panic.
}

#[test]
/// Oversized input rejected custom max.
fn caller_frame_limit_rejects_larger_input() {
    // Verify a custom (small) max_frame_size is enforced.
    let msg = ClientMessage::Input {
        data: vec![0x41; 1000],
    };
    let mut buf = Vec::new();
    write_message(&mut buf, &msg).unwrap();

    let result: Result<ClientMessage, FramingError> = read_message(&mut buf.as_slice(), 64);
    // The actual bincode payload for 1000 bytes of input will be > 64 bytes.
    assert!(
        matches!(result, Err(FramingError::Oversized { .. })),
        "expected Oversized with small max_frame_size"
    );
}

// ---- FrameData ↔ ratatui Buffer conversion ----

#[test]
/// Read message rejects trailing bytes.
fn extra_bytes_inside_a_frame_are_rejected() {
    // Encode a valid message, then append an extra byte after it.
    let msg = ClientMessage::Detach;
    let mut payload = bincode::serde::encode_to_vec(&msg, bincode::config::standard()).unwrap();
    let original_len = payload.len();
    payload.push(0xDE); // trailing garbage

    // Frame it with the inflated length (original + 1).
    let mut buf = u32::try_from(payload.len()).unwrap().to_le_bytes().to_vec();
    buf.extend_from_slice(&payload);

    let result: Result<ClientMessage, FramingError> =
        read_message(&mut buf.as_slice(), MAX_FRAME_SIZE);
    match result {
        Err(FramingError::Bincode(msg)) => {
            assert!(
                msg.contains("trailing bytes"),
                "error should mention trailing bytes: {msg}"
            );
            assert!(
                msg.contains(&format!("decoded {original_len}")),
                "error should mention decoded byte count: {msg}"
            );
        }
        other => panic!("expected Bincode error about trailing bytes, got: {other:?}"),
    }
}

#[test]
/// Write message accepts normal payload.
fn write_message_accepts_normal_payload() {
    // A normal detach payload can be written successfully.
    let msg = ClientMessage::Detach;
    let mut buf = Vec::new();
    assert!(write_message(&mut buf, &msg).is_ok());
}

// ---- Unix socketpair integration test ----

#[cfg(unix)]
#[test]
/// Framing over unix socketpair.
fn socket_pair_delivers_the_written_message_sequence() {
    use std::os::unix::net::UnixStream;

    let (mut a, mut b) = UnixStream::pair().expect("socketpair");

    let messages = vec![
        ClientMessage::Hello {
            version: PROTOCOL_VERSION,
            cols: 200,
            rows: 60,
            cell_width_px: 8,
            cell_height_px: 16,
            requested_encoding: RenderEncoding::SemanticFrame,
            keybindings: ClientKeybindings::Server,
            launch_mode: ClientLaunchMode::App,
        },
        ClientMessage::Input {
            data: b"hello world".to_vec(),
        },
        ClientMessage::ClipboardImage {
            extension: "png".to_owned(),
            data: vec![0x89, b'P', b'N', b'G'],
        },
        ClientMessage::Resize {
            cols: 100,
            rows: 30,
            cell_width_px: 8,
            cell_height_px: 16,
        },
        ClientMessage::Detach,
    ];

    // Keep both sockets blocking for this small sequential exchange.
    a.set_nonblocking(false).unwrap();
    b.set_nonblocking(false).unwrap();

    for msg in &messages {
        write_message(&mut a, msg).unwrap();
    }

    for expected in &messages {
        let decoded: ClientMessage = read_message(&mut b, MAX_FRAME_SIZE).unwrap();
        assert_eq!(*expected, decoded);
    }
}

// ---- Helper: chunked reader for simulating partial reads ----

/// A `Read` wrapper that yields at most `chunk_size` bytes per `read()` call,
/// simulating partial reads on a real socket.
struct ChunkedReader {
    /// Data.
    data: Vec<u8>,
    /// Pos.
    pos: usize,
    /// Chunk size.
    chunk_size: usize,
}

impl ChunkedReader {
    /// New.
    fn new(data: Vec<u8>, chunk_size: usize) -> Self {
        Self {
            data,
            pos: 0,
            chunk_size,
        }
    }
}

impl Read for ChunkedReader {
    /// Read.
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.data.len() {
            return Ok(0);
        }
        let remaining = self.data.len() - self.pos;
        let to_read = buf.len().min(remaining).min(self.chunk_size);
        buf[..to_read].copy_from_slice(&self.data[self.pos..self.pos + to_read]);
        self.pos += to_read;
        Ok(to_read)
    }
}

/// Styled wire cells with an indexed hyperlink.
fn varied_cells() -> Vec<CellData> {
    vec![
        CellData {
            symbol: "H".into(),
            fg: 2,
            bg: 1,
            modifier: 1,
            skip: false,
            hyperlink: None,
        },
        CellData {
            symbol: "i".into(),
            fg: 3,
            bg: 0,
            modifier: 4,
            skip: false,
            hyperlink: None,
        },
        CellData {
            symbol: "!".into(),
            fg: 0x02_ff_80_00,
            bg: 0x01_00_00_dc,
            modifier: 9,
            skip: false,
            hyperlink: Some(0),
        },
        CellData {
            symbol: " ".into(),
            fg: 0,
            bg: 0,
            modifier: 0,
            skip: true,
            hyperlink: None,
        },
        CellData {
            symbol: "→".into(), // multi-byte grapheme
            fg: 7,
            bg: 5,
            modifier: 32,
            skip: false,
            hyperlink: None,
        },
        CellData {
            symbol: "🦀".into(), // emoji, wide grapheme cluster
            fg: 4,
            bg: 6,
            modifier: 0,
            skip: false,
            hyperlink: None,
        },
    ]
}

/// Standard semantic-frame handshake.
fn hello_message() -> ClientMessage {
    ClientMessage::Hello {
        version: PROTOCOL_VERSION,
        cols: 80,
        rows: 24,
        cell_width_px: 8,
        cell_height_px: 16,
        requested_encoding: RenderEncoding::SemanticFrame,
        keybindings: ClientKeybindings::Server,
        launch_mode: ClientLaunchMode::App,
    }
}

/// Named modifier values preserve the existing wire bit layout.
#[test]
fn modifier_bits_match_wire_layout() {
    assert_eq!(
        [
            KeyModifiers::NONE,
            KeyModifiers::SHIFT,
            KeyModifiers::CONTROL,
            KeyModifiers::ALT,
            KeyModifiers::SUPER,
            KeyModifiers::HYPER,
            KeyModifiers::META
        ],
        [0, 1, 2, 4, 8, 16, 32]
    );
    assert_eq!(
        KeyModifiers::SHIFT | KeyModifiers::ALT | KeyModifiers::META,
        37
    );
}
