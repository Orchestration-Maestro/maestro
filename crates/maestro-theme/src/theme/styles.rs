//! Text decorations and the border callbacks that keep their prepared theme.
use super::{LiveTheme, Theme, ThemeColor};
use owo_colors::Style;
use std::rc::Rc;

/// Wrap `text` in `open` and `close`, keeping every nested `close` effective.
///
/// Enabled, nonempty text is changed: a matching close inside it is followed by `open`, and
/// each line feed is preceded by `close` and followed by `open`, with a carriage return
/// directly before the line feed kept before it. Other text is returned unchanged.
fn decorate(text: &str, enabled: bool, open: Style, close: &str) -> String {
    if !enabled || text.is_empty() {
        return text.to_owned();
    }
    let open = open.prefix_formatter().to_string();
    let reopened = text.replace(close, &format!("{close}{open}"));
    let mut out = open.clone();
    let mut rest = reopened.as_str();
    while let Some(index) = rest.find('\n') {
        let (line, eol) = match rest[..index].strip_suffix('\r') {
            Some(line) => (line, "\r\n"),
            None => (&rest[..index], "\n"),
        };
        out.extend([line, close, eol, &open]);
        rest = &rest[index + 1..];
    }
    out.extend([rest, close]);
    out
}

/// Style `text` with a foreground of the published theme, or leave it unstyled when
/// the theme is not initialized or lacks the key.
pub(super) fn paint(live: &LiveTheme, key: &ThemeColor, text: &str) -> String {
    live.get()
        .and_then(|theme| theme.fg(key, text))
        .unwrap_or_else(|_| text.to_owned())
}

impl Theme {
    /// Apply bold when `enabled`; text that is empty or not enabled is unchanged.
    #[must_use]
    pub fn bold(text: &str, enabled: bool) -> String {
        decorate(text, enabled, Style::new().bold(), "\x1b[22m")
    }

    /// Apply italic when `enabled`; text that is empty or not enabled is unchanged.
    #[must_use]
    pub fn italic(text: &str, enabled: bool) -> String {
        decorate(text, enabled, Style::new().italic(), "\x1b[23m")
    }

    /// Apply underline when `enabled`; text that is empty or not enabled is unchanged.
    #[must_use]
    pub fn underline(text: &str, enabled: bool) -> String {
        decorate(text, enabled, Style::new().underline(), "\x1b[24m")
    }

    /// Apply inverse video when `enabled`; text that is empty or not enabled is unchanged.
    #[must_use]
    pub fn inverse(text: &str, enabled: bool) -> String {
        decorate(text, enabled, Style::new().reversed(), "\x1b[27m")
    }

    /// Apply strikethrough when `enabled`; text that is empty or not enabled is unchanged.
    #[must_use]
    pub fn strikethrough(text: &str, enabled: bool) -> String {
        decorate(text, enabled, Style::new().strikethrough(), "\x1b[29m")
    }

    /// Return a callback coloring text with the thinking key of `level`.
    ///
    /// An unknown level uses `thinkingOff`. The callback keeps this instance, whatever
    /// is published later, and returns its text unstyled when the key is missing.
    #[must_use]
    pub fn get_thinking_border_color(self: &Rc<Self>, level: &str) -> Rc<dyn Fn(&str) -> String> {
        self.border(match level {
            "minimal" => ThemeColor::ThinkingMinimal,
            "low" => ThemeColor::ThinkingLow,
            "medium" => ThemeColor::ThinkingMedium,
            "high" => ThemeColor::ThinkingHigh,
            "xhigh" => ThemeColor::ThinkingXhigh,
            _ => ThemeColor::ThinkingOff,
        })
    }

    /// Return a callback coloring text with `bashMode` that keeps this instance.
    #[must_use]
    pub fn get_bash_mode_border_color(self: &Rc<Self>) -> Rc<dyn Fn(&str) -> String> {
        self.border(ThemeColor::BashMode)
    }

    /// Callback coloring text with `key` of this instance.
    fn border(self: &Rc<Self>, key: ThemeColor) -> Rc<dyn Fn(&str) -> String> {
        let theme = Rc::clone(self);
        Rc::new(move |text| theme.fg(&key, text).unwrap_or_else(|_| text.to_owned()))
    }
}
