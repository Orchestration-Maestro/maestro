//! Select-graphic-rendition decoding through borrowed parameter views.

use maestro_terminal::sgr::{Attribute, Parser};

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
        A::UnderlineColor(v) => ("underline_color", rgb_value(v)),
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
        A::DirectColorFg(v) => ("direct_color_fg", rgb_value(v)),
        A::DirectColorBg(v) => ("direct_color_bg", rgb_value(v)),
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

/// Preserve all RGB channels in the recorded representation.
fn rgb_value(v: maestro_terminal::color::Rgb) -> Value {
    json!({"r":v.r,"g":v.g,"b":v.b})
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
}
