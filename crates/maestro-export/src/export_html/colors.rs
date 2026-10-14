//! Background derivation from supplied resolved theme colors.
use maestro_text::is_whitespace_scalar;
use maestro_theme::ThemeExportColors;

/// Three independent document backgrounds.
pub(super) struct Backgrounds {
    /// Page color.
    pub page: String,
    /// Card color.
    pub card: String,
    /// Info color.
    pub info: String,
}
/// Resolve explicit backgrounds by presence, otherwise derive from the base color.
pub(super) fn backgrounds(colors: &[(String, String)], explicit: ThemeExportColors) -> Backgrounds {
    let base = colors
        .iter()
        .find(|(key, _)| key == "userMessageBg")
        .map(|(_, value)| value.as_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("#343541");
    let derived = derive(base);
    Backgrounds {
        page: explicit.page_bg.unwrap_or(derived.page),
        card: explicit.card_bg.unwrap_or(derived.card),
        info: explicit.info_bg.unwrap_or(derived.info),
    }
}
/// Parse full six-digit hex or lowercase RGB with shared whitespace.
fn parse(color: &str) -> Option<[f64; 3]> {
    if let Some(hex) = color
        .strip_prefix('#')
        .filter(|hex| hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return Some([0, 2, 4].map(|start| {
            f64::from(u8::from_str_radix(&hex[start..start + 2], 16).unwrap_or_default())
        }));
    }
    let rgb = color
        .strip_prefix("rgb")?
        .trim_start_matches(is_whitespace_scalar)
        .strip_prefix('(')?
        .strip_suffix(')')?;
    let mut parts = rgb.split(',');
    let mut channels = [0.0; 3];
    for channel in &mut channels {
        let part = parts.next()?.trim_matches(is_whitespace_scalar);
        if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        *channel = part.parse().ok()?;
    }
    if parts.next().is_some() {
        return None;
    }
    Some(channels)
}
/// Linearized channel used for luminance, retaining double expression order.
fn linear(channel: f64) -> f64 {
    let scaled = channel / 255.0;
    if scaled <= 0.03928 {
        scaled / 12.92
    } else {
        ((scaled + 0.055) / 1.055).powf(2.4)
    }
}
/// RGB spelling after brightness rounding and clamping.
fn brightness(channels: [f64; 3], factor: f64) -> String {
    rgb(channels.map(|channel| (channel * factor).round().clamp(0.0, 255.0)))
}
/// Emit integer-valued channels with the established CSS spacing.
fn rgb([red, green, blue]: [f64; 3]) -> String {
    format!("rgb({red}, {green}, {blue})")
}
/// Derive light or dark backgrounds, with malformed-color defaults.
fn derive(base: &str) -> Backgrounds {
    let Some([red, green, blue]) = parse(base) else {
        return Backgrounds {
            page: "rgb(24, 24, 30)".into(),
            card: "rgb(30, 30, 36)".into(),
            info: "rgb(60, 55, 40)".into(),
        };
    };
    let channels = [red, green, blue];
    if 0.2126 * linear(red) + 0.7152 * linear(green) + 0.0722 * linear(blue) > 0.5 {
        Backgrounds {
            page: brightness(channels, 0.96),
            card: base.into(),
            info: rgb([
                (red + 10.0).min(255.0),
                (green + 5.0).min(255.0),
                (blue - 20.0).max(0.0),
            ]),
        }
    } else {
        Backgrounds {
            page: brightness(channels, 0.7),
            card: brightness(channels, 0.85),
            info: rgb([(red + 20.0).min(255.0), (green + 15.0).min(255.0), blue]),
        }
    }
}
