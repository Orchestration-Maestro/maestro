//! Chrome palettes: the built-in themes, authored colour overrides and the
//! active theme chosen from the daemon's theme configuration and the host
//! terminal's appearance.

#[cfg(test)]
mod tests;
mod themes;

use maestro_app::daemon::{CustomThemeColors, canonical_theme_name};
#[cfg(target_os = "linux")]
use maestro_app::daemon::{HostAppearance, ThemeRuntimeConfig};
use ratatui::style::Color;

/// Every colour the multiplexer chrome uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    /// Primary accent: highlights and active borders.
    pub accent: Color,
    /// Background of the tab bar, floating panels, overlays and modals.
    pub panel_bg: Color,
    /// Desktop sidebar background; reset keeps the terminal background.
    pub sidebar_bg: Color,
    /// Background of the active workspace and focused agent rows.
    pub active_row_bg: Color,
    /// Background of the navigation cursor row in the sidebar.
    pub selection_bg: Color,
    /// Subtle surface behind selected or focused items.
    pub surface0: Color,
    /// Slightly lighter surface for hover and active states.
    pub surface1: Color,
    /// Very dim surface for separators.
    pub surface_dim: Color,
    /// Muted text: secondary information and numbers.
    pub overlay0: Color,
    /// Slightly brighter muted text.
    pub overlay1: Color,
    /// Main text.
    pub text: Color,
    /// Subdued text: workspace numbers and dim labels.
    pub subtext0: Color,
    /// Branch names and special labels.
    pub mauve: Color,
    /// Done and idle states.
    pub green: Color,
    /// Working and running states.
    pub yellow: Color,
    /// States that need attention or are blocked.
    pub red: Color,
    /// Unseen or done notification accent.
    pub blue: Color,
    /// Notification accent and unseen markers.
    pub teal: Color,
    /// Interrupted and warning states.
    pub peach: Color,
}

impl Palette {
    /// The built-in palette for a theme name or alias; unknown names have
    /// none.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match canonical_theme_name(name)? {
            "catppuccin" => Self::catppuccin(),
            "catppuccin-latte" => Self::catppuccin_latte(),
            "terminal" => Self::terminal(),
            "tokyo-night" => Self::tokyo_night(),
            "tokyo-night-day" => Self::tokyo_night_day(),
            "dracula" => Self::dracula(),
            "nord" => Self::nord(),
            "gruvbox" => Self::gruvbox(),
            "gruvbox-light" => Self::gruvbox_light(),
            "one-dark" => Self::one_dark(),
            "one-light" => Self::one_light(),
            "solarized" => Self::solarized(),
            "solarized-light" => Self::solarized_light(),
            "kanagawa" => Self::kanagawa(),
            "kanagawa-lotus" => Self::kanagawa_lotus(),
            "rose-pine" => Self::rose_pine(),
            "rose-pine-dawn" => Self::rose_pine_dawn(),
            "vesper" => Self::vesper(),
            _ => return None,
        })
    }

    /// Apply authored colour overrides on top of this palette.
    #[must_use]
    pub fn with_overrides(mut self, custom: &CustomThemeColors) -> Self {
        let tokens = [
            (&custom.accent, &mut self.accent),
            (&custom.panel_bg, &mut self.panel_bg),
            (&custom.sidebar_bg, &mut self.sidebar_bg),
            (&custom.active_row_bg, &mut self.active_row_bg),
            (&custom.selection_bg, &mut self.selection_bg),
            (&custom.surface0, &mut self.surface0),
            (&custom.surface1, &mut self.surface1),
            (&custom.surface_dim, &mut self.surface_dim),
            (&custom.overlay0, &mut self.overlay0),
            (&custom.overlay1, &mut self.overlay1),
            (&custom.text, &mut self.text),
            (&custom.subtext0, &mut self.subtext0),
            (&custom.mauve, &mut self.mauve),
            (&custom.green, &mut self.green),
            (&custom.yellow, &mut self.yellow),
            (&custom.red, &mut self.red),
            (&custom.blue, &mut self.blue),
            (&custom.teal, &mut self.teal),
            (&custom.peach, &mut self.peach),
        ];
        for (value, token) in tokens {
            if let Some(value) = value {
                *token = parse_color(value);
            }
        }
        self
    }
}

/// Parse an authored colour, ignoring case and surrounding spaces: a reset
/// alias, `#rrggbb`, `#rgb`, `rgb(r, g, b)` or a named colour; anything else
/// is logged and becomes cyan.
#[must_use]
pub fn parse_color(value: &str) -> Color {
    let value = value.trim().to_lowercase();
    if matches!(value.as_str(), "reset" | "default" | "none" | "transparent") {
        return Color::Reset;
    }
    value
        .strip_prefix('#')
        .and_then(parse_hex)
        .or_else(|| {
            value
                .strip_prefix("rgb(")
                .and_then(|inner| inner.strip_suffix(')'))
                .and_then(parse_rgb)
        })
        .or_else(|| named_color(&value))
        .unwrap_or_else(|| {
            tracing::warn!(color = value.as_str(), "unknown color, defaulting to cyan");
            Color::Cyan
        })
}

/// Six or three hexadecimal digits, each short digit repeated.
fn parse_hex(hex: &str) -> Option<Color> {
    match hex.len() {
        6 => {
            let channel = |at: usize| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok();
            Some(Color::Rgb(channel(0)?, channel(2)?, channel(4)?))
        }
        3 => {
            let digits: Vec<u8> = hex
                .chars()
                .filter_map(|digit| digit.to_digit(16))
                .filter_map(|digit| u8::try_from(digit * 17).ok())
                .collect();
            match digits[..] {
                [r, g, b] => Some(Color::Rgb(r, g, b)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Exactly three comma-separated byte values.
fn parse_rgb(inner: &str) -> Option<Color> {
    let mut parts = inner.split(',').map(|part| part.trim().parse::<u8>().ok());
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(Some(r)), Some(Some(g)), Some(Some(b)), None) => Some(Color::Rgb(r, g, b)),
        _ => None,
    }
}

/// A named terminal colour.
fn named_color(name: &str) -> Option<Color> {
    Some(match name {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" | "purple" => Color::Magenta,
        "cyan" => Color::Cyan,
        "white" => Color::White,
        "gray" | "grey" => Color::Gray,
        "darkgray" | "darkgrey" => Color::DarkGray,
        "lightred" => Color::LightRed,
        "lightgreen" => Color::LightGreen,
        "lightyellow" => Color::LightYellow,
        "lightblue" => Color::LightBlue,
        "lightmagenta" => Color::LightMagenta,
        "lightcyan" => Color::LightCyan,
        _ => return None,
    })
}

/// The palette for a theme name; an unknown name is logged and falls back to
/// `fallback_name`, then to Catppuccin. Authored overrides apply next, then
/// the legacy interface accent.
#[cfg(target_os = "linux")]
#[must_use]
fn resolve_palette_for_theme_name(
    name: &str,
    fallback_name: &str,
    runtime: &ThemeRuntimeConfig,
) -> Palette {
    let mut palette = Palette::from_name(name).unwrap_or_else(|| {
        tracing::warn!(
            theme = name,
            fallback = fallback_name,
            "unknown theme, falling back"
        );
        Palette::from_name(fallback_name).unwrap_or_else(Palette::catppuccin)
    });
    if let Some(custom) = &runtime.custom {
        palette = palette.with_overrides(custom);
    }
    if let Some(accent) = &runtime.legacy_accent {
        palette.accent = parse_color(accent);
    }
    palette
}

/// The active palette and theme name for a host appearance; automatic
/// switching treats an unreported appearance as dark.
#[cfg(target_os = "linux")]
#[must_use]
pub fn resolve_effective_theme(
    runtime: &ThemeRuntimeConfig,
    appearance: Option<HostAppearance>,
) -> (Palette, String) {
    let (name, fallback_name) = runtime.effective_selection(appearance);
    (
        resolve_palette_for_theme_name(name, fallback_name, runtime),
        name.to_owned(),
    )
}
