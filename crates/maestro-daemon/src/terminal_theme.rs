//! Retained host colours, appearance inference and terminal colour emission.

/// Shared host colour records; parsing and queries remain toolkit-owned.
pub use maestro_tui::terminal_theme::{DefaultColorKind, RgbColor};

/// Appearance inferred from or reported by the host terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostAppearance {
    /// Dark host background.
    Dark,
    /// Light host background.
    Light,
}

impl HostAppearance {
    /// Emit the host appearance as a colour-scheme report.
    #[must_use]
    pub const fn color_scheme_report(self) -> &'static [u8] {
        match self {
            Self::Dark => b"\x1b[?997;1n",
            Self::Light => b"\x1b[?997;2n",
        }
    }
}

/// Infer appearance from the weighted luminance of a host background colour.
#[must_use]
pub fn inferred_appearance(color: RgbColor) -> HostAppearance {
    let luminance = u32::from(color.r) * 299 + u32::from(color.g) * 587 + u32::from(color.b) * 114;
    if luminance >= 128_000 {
        HostAppearance::Light
    } else {
        HostAppearance::Dark
    }
}

/// Retained host default colours and independently supplied palette entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalTheme {
    /// Default text colour, if observed.
    pub foreground: Option<RgbColor>,
    /// Default background colour, if observed.
    pub background: Option<RgbColor>,
    /// Observed palette colours by index.
    pub palette: [Option<RgbColor>; 256],
}

impl Default for TerminalTheme {
    fn default() -> Self {
        Self {
            foreground: None,
            background: None,
            palette: [None; 256],
        }
    }
}

/// Query the host's colour scheme.
pub const HOST_COLOR_SCHEME_QUERY_SEQUENCE: &str = "\x1b[?996n";
/// Enable host colour-scheme reports.
pub const HOST_COLOR_SCHEME_REPORT_ENABLE_SEQUENCE: &str = "\x1b[?2031h";
/// Disable host colour-scheme reports.
pub const HOST_COLOR_SCHEME_REPORT_DISABLE_SEQUENCE: &str = "\x1b[?2031l";

impl TerminalTheme {
    /// Retain one observed default colour.
    #[must_use]
    pub fn with_color(mut self, kind: DefaultColorKind, color: RgbColor) -> Self {
        match kind {
            DefaultColorKind::Foreground => self.foreground = Some(color),
            DefaultColorKind::Background => self.background = Some(color),
        }
        self
    }

    /// Retain or replace one observed palette entry.
    #[must_use]
    pub fn with_palette_color(mut self, index: u8, color: RgbColor) -> Self {
        self.palette[usize::from(index)] = Some(color);
        self
    }

    /// Whether both default colours are absent, independently of palette entries.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.foreground.is_none() && self.background.is_none()
    }
}

/// Emit an eight-bit default colour using lower-case two-digit RGB channels.
#[must_use]
pub fn osc_set_default_color_sequence(kind: DefaultColorKind, color: RgbColor) -> String {
    let command = match kind {
        DefaultColorKind::Foreground => 10,
        DefaultColorKind::Background => 11,
    };
    format!(
        "\x1b]{command};rgb:{:02x}/{:02x}/{:02x}\x1b\\",
        color.r, color.g, color.b
    )
}

/// Reset one host default colour using its xterm OSC command.
#[must_use]
pub fn osc_reset_default_color_sequence(kind: DefaultColorKind) -> &'static str {
    match kind {
        DefaultColorKind::Foreground => "\x1b]110\x1b\\",
        DefaultColorKind::Background => "\x1b]111\x1b\\",
    }
}

#[cfg(test)]
mod tests;
