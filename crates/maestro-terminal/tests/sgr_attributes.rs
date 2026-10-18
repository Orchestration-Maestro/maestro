//! Select-graphic-rendition decoding through borrowed parameter views.

use maestro_terminal::sgr::{Attribute, Parser};

use crate::support::parsing::rgb;

#[test]
fn decode_sgr_parser() {
    let mut parser = Parser::EMPTY;
    assert_eq!(parser.next(), Some(Attribute::Unset));
    assert_eq!(parser.next(), None);
    assert_eq!(parser.next(), None);
}

use maestro_terminal::parser::SepList;
use serde::Deserialize;
use serde_json::{Value, json};

/// Recorded parameters and the complete ordered attributes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Input parameters.
    params: Vec<u16>,
    /// Positional colon bits.
    mask: u32,
    /// Every returned attribute, including both unknown slices.
    value: Vec<Value>,
}

/// Project one attribute without dropping payload identity or ordering.
fn attribute_value(attribute: Attribute<'_>) -> Value {
    use Attribute as A;
    let (tag, payload) = match attribute {
        A::Unset => ("unset", json!({})),
        A::Unknown(v) => ("unknown", json!({"full": v.full, "partial": v.partial})),
        A::Bold => ("bold", json!({})),
        A::ResetBold => ("reset_bold", json!({})),
        A::Italic => ("italic", json!({})),
        A::ResetItalic => ("reset_italic", json!({})),
        A::Faint => ("faint", json!({})),
        A::Underline(v) => (
            "underline",
            json!(match v {
                maestro_terminal::sgr::Underline::None => "none",
                maestro_terminal::sgr::Underline::Single => "single",
                maestro_terminal::sgr::Underline::Double => "double",
                maestro_terminal::sgr::Underline::Curly => "curly",
                maestro_terminal::sgr::Underline::Dotted => "dotted",
                maestro_terminal::sgr::Underline::Dashed => "dashed",
            }),
        ),
        A::UnderlineColor(v) => ("underline_color", rgb(v)),
        A::Palette256UnderlineColor(v) => ("256_underline_color", json!(v)),
        A::ResetUnderlineColor => ("reset_underline_color", json!({})),
        A::Overline => ("overline", json!({})),
        A::ResetOverline => ("reset_overline", json!({})),
        A::Blink => ("blink", json!({})),
        A::ResetBlink => ("reset_blink", json!({})),
        A::Inverse => ("inverse", json!({})),
        A::ResetInverse => ("reset_inverse", json!({})),
        A::Invisible => ("invisible", json!({})),
        A::ResetInvisible => ("reset_invisible", json!({})),
        A::Strikethrough => ("strikethrough", json!({})),
        A::ResetStrikethrough => ("reset_strikethrough", json!({})),
        A::DirectColorFg(v) => ("direct_color_fg", rgb(v)),
        A::DirectColorBg(v) => ("direct_color_bg", rgb(v)),
        A::Color8Bg(v) => ("8_bg", name_value(v)),
        A::Color8Fg(v) => ("8_fg", name_value(v)),
        A::ResetFg => ("reset_fg", json!({})),
        A::ResetBg => ("reset_bg", json!({})),
        A::Color8BrightBg(v) => ("8_bright_bg", name_value(v)),
        A::Color8BrightFg(v) => ("8_bright_fg", name_value(v)),
        A::Palette256Bg(v) => ("256_bg", json!(v)),
        A::Palette256Fg(v) => ("256_fg", json!(v)),
    };
    json!({tag: payload})
}

/// Render the source's named palette alternatives.
fn name_value(v: maestro_terminal::color::Name) -> Value {
    let names = [
        "black",
        "red",
        "green",
        "yellow",
        "blue",
        "magenta",
        "cyan",
        "white",
        "bright_black",
        "bright_red",
        "bright_green",
        "bright_yellow",
        "bright_blue",
        "bright_magenta",
        "bright_cyan",
        "bright_white",
    ];
    json!(names[usize::from(v.0)])
}

/// Assert every parameter group against its complete recorded outcome.
fn assert_cases(text: &str) {
    let rows: Vec<Case> = serde_json::from_str(text).unwrap();
    assert!(!rows.is_empty());
    for (index, row) in rows.into_iter().enumerate() {
        let parser = Parser {
            params: &row.params,
            params_sep: SepList(row.mask),
            idx: 0,
        };
        let actual: Vec<Value> = parser.map(attribute_value).collect();
        assert_eq!(actual, row.value, "case {index}");
    }
}

#[test]
fn sgr_colon_masks_keep_group_boundaries() {
    assert_cases(include_str!("fixtures/sgr-groups.json"));
}

#[test]
fn sgr_scalar_codes_and_truncation_match() {
    assert_cases(include_str!("fixtures/sgr-scalars.json"));
    for (value, byte) in [(255, 255), (256, 0), (65535, 255)] {
        assert_attributes(
            &[38, 5, value, 1],
            0,
            &[Attribute::Palette256Fg(byte), Attribute::Bold],
        );
        assert_attributes(
            &[48, 5, value, 1],
            0,
            &[Attribute::Palette256Bg(byte), Attribute::Bold],
        );
        assert_attributes(
            &[58, 5, value, 1],
            0,
            &[Attribute::Palette256UnderlineColor(byte), Attribute::Bold],
        );
    }
    let rgb = Rgb {
        r: 255,
        g: 0,
        b: 255,
    };
    assert_attributes(
        &[38, 2, 255, 256, 65535, 1],
        0,
        &[Attribute::DirectColorFg(rgb), Attribute::Bold],
    );
    assert_attributes(
        &[48, 2, 255, 256, 65535, 1],
        0,
        &[Attribute::DirectColorBg(rgb), Attribute::Bold],
    );
    assert_attributes(
        &[58, 2, 255, 256, 65535, 1],
        0,
        &[Attribute::UnderlineColor(rgb), Attribute::Bold],
    );
}

use maestro_terminal::{
    color::{Name, Rgb},
    sgr::{Underline, Unknown},
};

/// Assert the complete ordered stream, including repeated exhaustion.
fn assert_attributes(params: &[u16], mask: u32, expected: &[Attribute<'_>]) {
    let mut parser = Parser {
        params,
        params_sep: SepList(mask),
        idx: 0,
    };
    assert_eq!(parser.by_ref().collect::<Vec<_>>(), expected);
    assert_eq!(parser.next(), None);
    assert_eq!(parser.next(), None);
}

#[test]
fn decode_sgr_parser_multiple() {
    assert_attributes(
        &[0, 38, 2, 40, 44, 52],
        0,
        &[
            Attribute::Unset,
            Attribute::DirectColorFg(Rgb {
                r: 40,
                g: 44,
                b: 52,
            }),
        ],
    );
}

#[test]
fn decode_sgr_unsupported_with_colon() {
    let params = [0, 4, 1];
    assert_attributes(
        &params,
        1,
        &[
            Attribute::Unknown(Unknown {
                full: &params,
                partial: &params[..2],
            }),
            Attribute::Bold,
        ],
    );
}

#[test]
fn decode_sgr_unsupported_with_multiple_colon() {
    let params = [0, 4, 2, 1];
    assert_attributes(
        &params,
        3,
        &[
            Attribute::Unknown(Unknown {
                full: &params,
                partial: &params[..3],
            }),
            Attribute::Bold,
        ],
    );
}

#[test]
fn decode_sgr_underline_style_with_too_many_colons() {
    let params = [4, 2, 3, 1];
    assert_attributes(
        &params,
        3,
        &[
            Attribute::Unknown(Unknown {
                full: &params,
                partial: &params[..],
            }),
            Attribute::Bold,
        ],
    );
}

#[test]
fn decode_sgr_bold() {
    assert_attributes(&[1, 22], 0, &[Attribute::Bold, Attribute::ResetBold]);
}

#[test]
fn decode_sgr_italic() {
    assert_attributes(&[3, 23], 0, &[Attribute::Italic, Attribute::ResetItalic]);
}

#[test]
fn decode_sgr_blink() {
    assert_attributes(
        &[5, 6, 25],
        0,
        &[Attribute::Blink, Attribute::Blink, Attribute::ResetBlink],
    );
}

#[test]
fn decode_sgr_inverse() {
    assert_attributes(&[7, 27], 0, &[Attribute::Inverse, Attribute::ResetInverse]);
}

#[test]
fn decode_sgr_strikethrough() {
    assert_attributes(
        &[9, 29],
        0,
        &[Attribute::Strikethrough, Attribute::ResetStrikethrough],
    );
}

#[test]
fn decode_sgr_invisible() {
    assert_attributes(
        &[8, 28],
        0,
        &[Attribute::Invisible, Attribute::ResetInvisible],
    );
}

#[test]
fn decode_sgr_underline() {
    assert_attributes(
        &[4, 24],
        0,
        &[
            Attribute::Underline(Underline::Single),
            Attribute::Underline(Underline::None),
        ],
    );
}

#[test]
fn decode_sgr_underline_styles() {
    for (value, style) in [
        (0, Underline::None),
        (1, Underline::Single),
        (2, Underline::Double),
        (3, Underline::Curly),
        (4, Underline::Dotted),
        (5, Underline::Dashed),
        (6, Underline::Single),
    ] {
        assert_attributes(&[4, value], 1, &[Attribute::Underline(style)]);
    }
}

#[test]
fn decode_sgr_underline_style_with_more() {
    assert_attributes(
        &[4, 2, 1],
        1,
        &[Attribute::Underline(Underline::Double), Attribute::Bold],
    );
}

#[test]
fn decode_sgr_8_color() {
    assert_attributes(
        &[30, 37, 40, 47, 90, 97, 100, 107],
        0,
        &[
            Attribute::Color8Fg(Name(0)),
            Attribute::Color8Fg(Name(7)),
            Attribute::Color8Bg(Name(0)),
            Attribute::Color8Bg(Name(7)),
            Attribute::Color8BrightFg(Name(8)),
            Attribute::Color8BrightFg(Name(15)),
            Attribute::Color8BrightBg(Name(8)),
            Attribute::Color8BrightBg(Name(15)),
        ],
    );
}

#[test]
fn decode_sgr_256_color() {
    assert_attributes(
        &[38, 5, 161, 48, 5, 236],
        0,
        &[Attribute::Palette256Fg(161), Attribute::Palette256Bg(236)],
    );
}

#[test]
fn decode_sgr_256_color_underline() {
    assert_attributes(&[58, 5, 9], 0, &[Attribute::Palette256UnderlineColor(9)]);
}

#[test]
fn decode_sgr_24_bit_bg_color() {
    assert_attributes(
        &[48, 2, 1, 2, 3],
        15,
        &[Attribute::DirectColorBg(Rgb { r: 1, g: 2, b: 3 })],
    );
}

#[test]
fn decode_sgr_underline_color() {
    assert_attributes(
        &[58, 2, 1, 2, 3],
        15,
        &[Attribute::UnderlineColor(Rgb { r: 1, g: 2, b: 3 })],
    );
}

#[test]
fn decode_sgr_reset_underline_color() {
    assert_attributes(&[59], 0, &[Attribute::ResetUnderlineColor]);
}

#[test]
fn decode_sgr_underline_bg_and_fg() {
    assert_attributes(
        &[4, 38, 2, 255, 247, 219, 48, 2, 242, 93, 147, 4],
        0,
        &[
            Attribute::Underline(Underline::Single),
            Attribute::DirectColorFg(Rgb {
                r: 255,
                g: 247,
                b: 219,
            }),
            Attribute::DirectColorBg(Rgb {
                r: 242,
                g: 93,
                b: 147,
            }),
            Attribute::Underline(Underline::Single),
        ],
    );
}

#[test]
fn decode_sgr_direct_color_fg_missing_color() {
    let params = [38, 5];
    assert_attributes(
        &params,
        0,
        &[
            Attribute::Unknown(Unknown {
                full: &params,
                partial: &params,
            }),
            Attribute::Blink,
        ],
    );
}

#[test]
fn decode_sgr_direct_color_bg_missing_color() {
    let params = [48, 5];
    assert_attributes(
        &params,
        0,
        &[
            Attribute::Unknown(Unknown {
                full: &params,
                partial: &params,
            }),
            Attribute::Blink,
        ],
    );
}

#[test]
fn decode_sgr_direct_fg_bg_underline_ignore_optional_color_space() {
    for (code, build) in [
        (
            38,
            Attribute::DirectColorFg as fn(Rgb) -> Attribute<'static>,
        ),
        (48, Attribute::DirectColorBg),
        (58, Attribute::UnderlineColor),
    ] {
        assert_attributes(
            &[code, 2, 71, 1, 2, 3],
            31,
            &[build(Rgb { r: 1, g: 2, b: 3 })],
        );
        assert_attributes(
            &[code, 2, 0, 1, 2, 3],
            0,
            &[build(Rgb { r: 0, g: 1, b: 2 }), Attribute::Italic],
        );
    }
}

#[test]
fn decode_sgr_direct_fg_colon_with_too_many_colons() {
    let params = [38, 2, 0, 1, 2, 3, 4, 1];
    assert_attributes(
        &params,
        63,
        &[
            Attribute::Unknown(Unknown {
                full: &params,
                partial: &params,
            }),
            Attribute::Bold,
        ],
    );
}

#[test]
fn decode_sgr_direct_fg_colon_with_colorspace_and_extra_param() {
    assert_attributes(
        &[38, 2, 0, 1, 2, 3, 1],
        31,
        &[
            Attribute::DirectColorFg(Rgb { r: 1, g: 2, b: 3 }),
            Attribute::Bold,
        ],
    );
}

#[test]
fn decode_sgr_direct_fg_colon_no_colorspace_and_extra_param() {
    assert_attributes(
        &[38, 2, 1, 2, 3, 1],
        15,
        &[
            Attribute::DirectColorFg(Rgb { r: 1, g: 2, b: 3 }),
            Attribute::Bold,
        ],
    );
}

#[test]
fn decode_sgr_kakoune_input() {
    assert_attributes(
        &[0, 4, 3, 38, 2, 175, 175, 215, 58, 2, 0, 190, 80, 70],
        7938,
        &[
            Attribute::Unset,
            Attribute::Underline(Underline::Curly),
            Attribute::DirectColorFg(Rgb {
                r: 175,
                g: 175,
                b: 215,
            }),
            Attribute::UnderlineColor(Rgb {
                r: 190,
                g: 80,
                b: 70,
            }),
        ],
    );
}

#[test]
fn decode_sgr_kakoune_input_issue_underline_fg_and_bg() {
    assert_attributes(
        &[
            4, 3, 38, 2, 51, 51, 51, 48, 2, 170, 170, 170, 58, 2, 255, 97, 136,
        ],
        1,
        &[
            Attribute::Underline(Underline::Curly),
            Attribute::DirectColorFg(Rgb {
                r: 51,
                g: 51,
                b: 51,
            }),
            Attribute::DirectColorBg(Rgb {
                r: 170,
                g: 170,
                b: 170,
            }),
            Attribute::UnderlineColor(Rgb {
                r: 255,
                g: 97,
                b: 136,
            }),
        ],
    );
}

#[test]
fn decode_sgr_underline_colon_with_trailing_separator_and_short_slice() {
    let params = [58, 4];
    assert_attributes(
        &params,
        3,
        &[
            Attribute::Unknown(Unknown {
                full: &params,
                partial: &params,
            }),
            Attribute::Unknown(Unknown {
                full: &params,
                partial: &params[1..],
            }),
        ],
    );
}
