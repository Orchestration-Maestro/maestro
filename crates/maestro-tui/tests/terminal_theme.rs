//! Host terminal colour query and reply behaviour.
use maestro_tui::terminal_theme::{DefaultColorKind, RgbColor, parse_default_color_response};

#[test]
fn pane_parses_st_terminated_rgb_response() {
    let parsed = parse_default_color_response("\x1b]10;rgb:cccc/dddd/eeee\x1b\\");
    assert_eq!(
        parsed,
        Some((
            DefaultColorKind::Foreground,
            RgbColor {
                r: 0xcc,
                g: 0xdd,
                b: 0xee
            }
        ))
    );
}

#[test]
fn pane_parses_bel_terminated_hash_response() {
    assert_eq!(
        parse_default_color_response("\x1b]11;#123456\u{7}"),
        Some((
            DefaultColorKind::Background,
            RgbColor {
                r: 0x12,
                g: 0x34,
                b: 0x56
            }
        ))
    );
}

#[test]
fn pane_parses_palette_responses_and_builds_full_query() {
    use maestro_tui::terminal_theme::{
        HOST_COLOR_QUERY_SEQUENCE, host_terminal_theme_query_sequence, parse_palette_color_response,
    };
    assert_eq!(
        parse_palette_color_response("\x1b]4;255;rgb:1111/2222/3333\x1b\\"),
        Some((
            255,
            RgbColor {
                r: 0x11,
                g: 0x22,
                b: 0x33
            }
        ))
    );
    let query = host_terminal_theme_query_sequence(true);
    assert!(query.starts_with(HOST_COLOR_QUERY_SEQUENCE));
    assert!(query.contains("\x1b]4;0;?\x1b\\"));
    assert!(query.ends_with("\x1b]4;255;?\x1b\\"));
    assert_eq!(query.matches("\x1b]4;").count(), 256);
    assert_eq!(
        host_terminal_theme_query_sequence(false),
        HOST_COLOR_QUERY_SEQUENCE
    );
}

#[test]
fn pane_scales_short_hex_components() {
    for (component, channel) in [("f", 255), ("80", 128), ("800", 128), ("8000", 128)] {
        let reply = format!("\x1b]10;rgb:{component}/{component}/{component}\x1b\\");
        assert_eq!(
            parse_default_color_response(&reply),
            Some((
                DefaultColorKind::Foreground,
                RgbColor {
                    r: channel,
                    g: channel,
                    b: channel
                }
            ))
        );
    }
}

#[test]
fn host_reply_forms_keep_channels_and_rounded_components() {
    use maestro_tui::terminal_theme::parse_palette_color_response;
    for (operand, [r, g, b]) in [
        ("rgb:f/0/8", [255, 0, 136]),
        ("rgba:f/0/8", [255, 0, 136]),
        ("rgb:7F/80/fF", [127, 128, 255]),
        ("rgba:7ff/800/fff", [127, 128, 255]),
        ("rgb:7fff/8000/FFFF", [127, 128, 255]),
        ("rgba:f/80/8000", [255, 128, 128]),
        ("#f08", [255, 0, 136]),
        ("#7F80fF", [127, 128, 255]),
        ("#7ff800fff", [127, 128, 255]),
        ("#7fff8000FFFF", [127, 128, 255]),
    ] {
        let expected = RgbColor { r, g, b };
        for terminator in ["\u{7}", "\x1b\\"] {
            for (command, kind) in [
                (10, DefaultColorKind::Foreground),
                (11, DefaultColorKind::Background),
            ] {
                let reply = format!("\x1b]{command};{operand}{terminator}");
                assert_eq!(parse_default_color_response(&reply), Some((kind, expected)));
            }
            for index in [0, 255] {
                let reply = format!("\x1b]4;{index};{operand}{terminator}");
                assert_eq!(
                    parse_palette_color_response(&reply),
                    Some((index, expected))
                );
            }
        }
    }
}

#[test]
fn malformed_unicode_default_reply_is_rejected_without_panicking() {
    assert_eq!(parse_default_color_response("\x1b]11;#éa\x1b\\"), None);
    assert_eq!(
        parse_default_color_response("\x1b]11;#123\x1b\\"),
        Some((
            DefaultColorKind::Background,
            RgbColor {
                r: 17,
                g: 34,
                b: 51
            }
        ))
    );
}

#[test]
fn malformed_unicode_palette_reply_is_rejected_without_panicking() {
    use maestro_tui::terminal_theme::parse_palette_color_response;
    assert_eq!(parse_palette_color_response("\x1b]4;1;#éa\x1b\\"), None);
    assert_eq!(
        parse_palette_color_response("\x1b]4;1;#123\x1b\\"),
        Some((
            1,
            RgbColor {
                r: 17,
                g: 34,
                b: 51
            }
        ))
    );
}

#[test]
fn host_reply_validation_rejects_malformed_neighbors() {
    use maestro_tui::terminal_theme::parse_palette_color_response;
    for operand in MALFORMED_OPERANDS {
        assert_eq!(
            parse_default_color_response(&format!("\x1b]10;{operand}\u{7}")),
            None
        );
        assert_eq!(
            parse_palette_color_response(&format!("\x1b]4;0;{operand}\u{7}")),
            None
        );
    }
    for reply in [
        "\x1b]12;#123\u{7}",
        "11;#123\u{7}",
        "\x1b]11;#123",
        "\x1b]11;#123\x1b",
        "\x1b]11;#123\u{7}tail",
    ] {
        assert_eq!(parse_default_color_response(reply), None);
    }
    for reply in [
        "\x1b]5;0;#123\u{7}",
        "4;0;#123\u{7}",
        "\x1b]4;0;#123",
        "\x1b]4;0;#123\u{7}tail",
        "\x1b]4;256;#123\u{7}",
        "\x1b]4;-1;#123\u{7}",
        "\x1b]4;;#123\u{7}",
    ] {
        assert_eq!(parse_palette_color_response(reply), None);
    }
    assert_eq!(
        parse_default_color_response("\x1b]10;rgba:1/2/3\u{7}"),
        Some((
            DefaultColorKind::Foreground,
            RgbColor {
                r: 17,
                g: 34,
                b: 51
            }
        ))
    );
    assert_eq!(
        parse_palette_color_response("\x1b]4;255;#123\u{7}"),
        Some((
            255,
            RgbColor {
                r: 17,
                g: 34,
                b: 51
            }
        ))
    );
}

/// Invalid operands shared by the default and palette reply boundaries.
const MALFORMED_OPERANDS: [&str; 15] = [
    "rgb:",
    "rgb:1/2",
    "rgb:1/2/3/4",
    "rgba:1/2/3/4",
    "rgb:/2/3",
    "rgb:1/g/3",
    "rgb:10000/2/3",
    "#12",
    "#1234",
    "#1234567",
    "#1234567890",
    "#123456789012345",
    "#12g",
    "#éa",
    "RGB:1/2/3",
];
