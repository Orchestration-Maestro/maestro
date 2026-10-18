//! Terminal colour decoding through its public byte interface.

use maestro_terminal::color::{Rgb, parse_palette_entry};
use serde::Deserialize;

/// Recorded colour channels.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Channels {
    /// Red.
    r: u8,
    /// Green.
    g: u8,
    /// Blue.
    b: u8,
}
impl From<Channels> for Rgb {
    fn from(c: Channels) -> Self {
        Self {
            r: c.r,
            g: c.g,
            b: c.b,
        }
    }
}

/// A byte input with either a value or an error category.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ColorCase<I = Vec<u8>> {
    /// Authored bytes.
    input: I,
    /// Expected successful channels.
    value: Option<Channels>,
    /// Expected failure category.
    err: Option<String>,
}

/// One recorded successful palette assignment.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryValue {
    /// Palette index.
    index: u8,
    /// Assigned channels.
    color: Channels,
}

/// One palette-assignment input and its outcome.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryCase {
    /// Authored bytes.
    input: Vec<u8>,
    /// Expected assignment.
    value: Option<EntryValue>,
    /// Expected failure category.
    err: Option<String>,
}

#[test]
fn rgb_width_intensity_and_whitespace_edges() {
    assert_eq!(
        Rgb::parse(b"#123456789").unwrap(),
        Rgb {
            r: 18,
            g: 69,
            b: 120
        }
    );
    assert_eq!(
        Rgb::parse(b"rgbi:0/0.5/1").unwrap(),
        Rgb {
            r: 0,
            g: 127,
            b: 255
        }
    );
    assert_eq!(
        Rgb::parse(b"rgb:010/0100/1").unwrap(),
        Rgb { r: 0, g: 0, b: 17 }
    );
}

#[test]
fn decode_rgb_parse() {
    let cases: Vec<ColorCase> =
        serde_json::from_str(include_str!("fixtures/rgb-colors.json")).unwrap();
    for (index, case) in cases.into_iter().enumerate() {
        let actual = Rgb::parse(&case.input);
        match (case.value, case.err) {
            (Some(value), None) => assert_eq!(actual.unwrap(), value.into(), "case {index}"),
            (None, Some(err)) => assert_eq!(actual.unwrap_err().to_string(), err, "case {index}"),
            _ => panic!("invalid colour fixture {index}"),
        }
    }
    let rgb = Rgb {
        r: 1,
        g: 35,
        b: 255,
    };
    assert!(rgb.eql(rgb));
    for other in [
        Rgb { r: 2, ..rgb },
        Rgb { g: 36, ..rgb },
        Rgb { b: 254, ..rgb },
    ] {
        assert!(!rgb.eql(other));
    }
}

#[test]
fn decode_parsepaletteentry() {
    let cases: Vec<EntryCase> =
        serde_json::from_str(include_str!("fixtures/palette-entries.json")).unwrap();
    for (index, case) in cases.into_iter().enumerate() {
        let actual = parse_palette_entry(&case.input);
        match (case.value, case.err) {
            (Some(value), None) => {
                let actual = actual.unwrap();
                assert_eq!(actual.index, value.index, "case {index}");
                assert_eq!(actual.color, value.color.into(), "case {index}");
            }
            (None, Some(err)) => assert_eq!(actual.unwrap_err().to_string(), err, "case {index}"),
            _ => panic!("invalid palette fixture {index}"),
        }
    }
}

#[test]
fn decode_entries() {
    let cases: Vec<ColorCase> =
        serde_json::from_str(include_str!("fixtures/x11-colors.json")).unwrap();
    let entries = maestro_terminal::x11_color::entries();
    assert_eq!(entries.len(), cases.len());
    assert_eq!(entries[0].name, "snow");
    for (entry, case) in entries.iter().zip(cases) {
        assert!(case.err.is_none());
        assert_eq!(entry.name.as_bytes(), case.input);
        assert_eq!(entry.color, case.value.unwrap().into());
        assert_eq!(
            maestro_terminal::x11_color::map().get(&case.input),
            Some(entry.color)
        );
    }
}

#[test]
fn colour_lookup_keeps_ascii_identity() {
    let map = maestro_terminal::x11_color::map();
    assert_eq!(
        map.get(b"FoReStGReen"),
        Some(Rgb {
            r: 34,
            g: 139,
            b: 34
        })
    );
    assert_eq!(
        map.get(b"medium spring green"),
        Some(Rgb {
            r: 0,
            g: 250,
            b: 154
        })
    );
    for name in [
        "\u{212a}haki",
        "red\u{a0}",
        "\u{feff}red",
        "red\n",
        "red\r",
        " red",
        "red ",
        "medium  spring green",
        "zz",
    ] {
        assert_eq!(map.get(name.as_bytes()), None);
    }
    assert_eq!(map.get(&[b'a'; 64]), None);
    assert_eq!(map.get(&[b'a'; 128]), None);
}

#[test]
fn decode_rgb_encode() {
    let rgb = Rgb {
        r: 1,
        g: 35,
        b: 255,
    };
    let mut bytes = Vec::new();
    rgb.encode_rgb8(&mut bytes).unwrap();
    assert_eq!(bytes, b"rgb:01/23/ff");
    bytes.clear();
    rgb.encode_rgb16(&mut bytes).unwrap();
    assert_eq!(bytes, b"rgb:0101/2323/ffff");
}

#[test]
fn palette_mask_edits_preserve_other_indices() {
    use maestro_terminal::color::{DEFAULT, DynamicPalette};
    let mut palette = DynamicPalette::init(DEFAULT);
    let replacement = Rgb {
        r: 1,
        g: 35,
        b: 255,
    };
    palette.set(0, replacement);
    palette.set(255, replacement);
    let next = [Rgb { r: 4, g: 5, b: 6 }; 256];
    palette.change_default(next);
    let mut expected = next;
    expected[0] = replacement;
    expected[255] = replacement;
    assert_eq!(palette.current, expected);
    assert_eq!(palette.original, next);
    for index in 0..=255_u8 {
        assert_eq!(palette.mask.is_set(index), matches!(index, 0 | 255));
    }
    palette.reset(0);
    assert!(!palette.mask.is_set(0));
    assert!(palette.mask.is_set(255));
    assert_eq!(palette.current[0], next[0]);
    assert_eq!(palette.current[255], replacement);
    palette.reset_all();
    assert_eq!(palette.current, next);
    assert_eq!(palette.original, next);
    for index in 0..=255 {
        assert!(!palette.mask.is_set(index));
    }
}

#[test]
fn dynamic_rgb_defaults_and_overrides_survive_edits() {
    use maestro_terminal::color::DynamicRgb;
    let red = Rgb { r: 255, g: 0, b: 0 };
    let blue = Rgb { r: 0, g: 0, b: 255 };
    let mut color = DynamicRgb::UNSET;
    assert_eq!(color.get(), None);
    color.default = Some(red);
    assert_eq!(color.get(), Some(red));
    color.set(blue);
    assert_eq!(color.get(), Some(blue));
    color.reset();
    assert_eq!(color.r#override, Some(red));
    color.default = Some(blue);
    assert_eq!(color.get(), Some(red));
    let init = DynamicRgb::init(blue);
    assert_eq!(init.default, Some(blue));
    assert_eq!(init.r#override, None);
}

#[test]
fn decode_color_osc4() {
    use maestro_terminal::color::Special;
    let colors = [
        Special::Bold,
        Special::Underline,
        Special::Blink,
        Special::Reverse,
        Special::Italic,
    ];
    assert_eq!(colors.map(Special::osc4), [256, 257, 258, 259, 260]);
}

#[test]
fn decode_color_next() {
    use maestro_terminal::color::Dynamic;
    let colors = [
        Dynamic::Foreground,
        Dynamic::Background,
        Dynamic::Cursor,
        Dynamic::PointerForeground,
        Dynamic::PointerBackground,
        Dynamic::TektronixForeground,
        Dynamic::TektronixBackground,
        Dynamic::HighlightBackground,
        Dynamic::TektronixCursor,
        Dynamic::HighlightForeground,
    ];
    for (index, color) in colors.into_iter().enumerate() {
        assert_eq!(usize::from(color as u8), index + 10);
        assert_eq!(color.next(), colors.get(index + 1).copied());
    }
}

#[test]
fn decode_palette_default() {
    let colors: Vec<Channels> =
        serde_json::from_str(include_str!("fixtures/default-palette.json")).unwrap();
    assert_eq!(colors.len(), 256);
    let expected: Vec<Rgb> = colors.into_iter().map(Into::into).collect();
    assert_eq!(maestro_terminal::color::DEFAULT.as_slice(), expected);
    assert_eq!(
        maestro_terminal::color::DynamicPalette::DEFAULT
            .current
            .as_slice(),
        expected
    );
    assert_eq!(
        maestro_terminal::color::DynamicPalette::default()
            .original
            .as_slice(),
        expected
    );
}

#[test]
fn colour_errors_and_writer_failures_keep_categories() {
    use maestro_terminal::color::{Name, ParseError};
    assert_eq!(ParseError::InvalidFormat.to_string(), "InvalidFormat");
    assert_eq!(ParseError::Overflow.to_string(), "Overflow");
    let rgb = Rgb {
        r: 1,
        g: 35,
        b: 255,
    };
    let mut rejecting = std::io::Cursor::new([0; 1]);
    assert_eq!(
        rgb.encode_rgb8(&mut rejecting).unwrap_err().kind(),
        std::io::ErrorKind::WriteZero
    );
    assert_eq!(
        rgb.encode_rgb16(&mut rejecting).unwrap_err().kind(),
        std::io::ErrorKind::WriteZero
    );
    let rows: Vec<ColorCase<u8>> =
        serde_json::from_str(include_str!("fixtures/named-defaults.json")).unwrap();
    assert_eq!(rows.len(), 256);
    for (index, row) in rows.into_iter().enumerate() {
        assert_eq!(usize::from(row.input), index);
        let actual = Name(row.input).default();
        match (row.value, row.err) {
            (Some(value), None) => assert_eq!(actual.unwrap(), value.into()),
            (None, Some(err)) => assert_eq!(actual.unwrap_err().to_string(), err),
            _ => panic!("invalid default fixture {index}"),
        }
    }
}
