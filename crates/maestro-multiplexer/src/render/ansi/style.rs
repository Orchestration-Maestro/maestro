//! Select Graphic Rendition sequences for packed frame-record styles.

use maestro_protocol::frames::underline_style_from_modifier;
use ratatui::style::Modifier;
use std::fmt::Write as _;

/// The colour layer an SGR parameter sets.
#[derive(Clone, Copy)]
pub(super) enum Layer {
    /// Text colour.
    Foreground,
    /// Cell background.
    Background,
}

impl Layer {
    /// Default, first named, first bright and extended-colour parameters.
    const fn codes(self) -> (u8, u8, u8, u8) {
        match self {
            Self::Foreground => (39, 30, 90, 38),
            Self::Background => (49, 40, 100, 48),
        }
    }
}

/// SGR parameters for a packed colour; unknown names and tags select the
/// layer's default colour.
pub(super) fn color_to_sgr(value: u32, layer: Layer) -> String {
    let (default, named, bright, extended) = layer.codes();
    let [tag, r, g, b] = value.to_be_bytes();
    match (tag, b) {
        (0x00, 1..=8) => (named + b - 1).to_string(),
        (0x00, 9..=16) => (bright + b - 9).to_string(),
        (0x01, _) => format!("{extended};5;{b}"),
        (0x02, _) => format!("{extended};2;{r};{g};{b}"),
        _ => default.to_string(),
    }
}

/// SGR parameters for packed modifier bits, in parameter order; underline
/// styles 2 to 5 use their colon subparameter.
pub(super) fn modifier_to_sgr_parts(value: u16) -> Vec<&'static str> {
    let modifier = Modifier::from_bits_retain(value);
    let underline = match underline_style_from_modifier(value) {
        2 => "4:2",
        3 => "4:3",
        4 => "4:4",
        5 => "4:5",
        _ => "4",
    };
    [
        (Modifier::BOLD, "1"),
        (Modifier::DIM, "2"),
        (Modifier::ITALIC, "3"),
        (Modifier::UNDERLINED, underline),
        (Modifier::SLOW_BLINK, "5"),
        (Modifier::RAPID_BLINK, "6"),
        (Modifier::REVERSED, "7"),
        (Modifier::HIDDEN, "8"),
        (Modifier::CROSSED_OUT, "9"),
    ]
    .into_iter()
    .filter_map(|(flag, part)| modifier.contains(flag).then_some(part))
    .collect()
}

/// One SGR sequence that resets the previous style, then sets a cell's
/// modifiers and colours.
pub(super) fn build_sgr(fg: u32, bg: u32, modifier: u16) -> String {
    let mut sgr = String::from("\x1b[0");
    for part in modifier_to_sgr_parts(modifier) {
        sgr.push(';');
        sgr.push_str(part);
    }
    let _ = write!(
        sgr,
        ";{};{}m",
        color_to_sgr(fg, Layer::Foreground),
        color_to_sgr(bg, Layer::Background)
    );
    sgr
}
