//! Literal HTML conversion of terminal SGR text.
use maestro_text::markup::{EscapeMode, escape_into};
use maestro_theme::ansi256_to_hex;

/// Retained foreground and background style.
#[derive(Default)]
struct TextStyle {
    /// Foreground CSS color.
    fg: Option<String>,
    /// Background CSS color.
    bg: Option<String>,
    /// Coupled bold and dim controls.
    intensity: Intensity,
    /// Italic face.
    italic: bool,
    /// Underlined decoration.
    underline: bool,
}
/// Weight and opacity, reset together by SGR 22.
#[derive(Default)]
struct Intensity {
    /// Bold weight.
    bold: bool,
    /// Dim opacity.
    dim: bool,
}
impl TextStyle {
    /// Apply complete SGR groups in order, leaving incomplete operands active.
    fn apply(&mut self, params: &str) {
        let codes: Vec<u32> = params.split(';').map(|p| p.parse().unwrap_or(0)).collect();
        let mut codes = codes.iter();
        while let Some(&code) = codes.next() {
            match code {
                0 => *self = Self::default(),
                1 => self.intensity.bold = true,
                2 => self.intensity.dim = true,
                3 => self.italic = true,
                4 => self.underline = true,
                22 => {
                    self.intensity.bold = false;
                    self.intensity.dim = false;
                }
                23 => self.italic = false,
                24 => self.underline = false,
                39 => self.fg = None,
                49 => self.bg = None,
                30..=37 => self.fg = Some(ansi256_to_hex(code - 30)),
                40..=47 => self.bg = Some(ansi256_to_hex(code - 40)),
                90..=97 => self.fg = Some(ansi256_to_hex(code - 90 + 8)),
                100..=107 => self.bg = Some(ansi256_to_hex(code - 100 + 8)),
                38 | 48 => self.extended(code, &mut codes),
                _ => {}
            }
        }
    }
    /// Update one color plane only when its extended operand group is complete.
    fn extended(&mut self, code: u32, codes: &mut std::slice::Iter<'_, u32>) {
        if let Some(color) = extended_color(codes) {
            let plane = if code == 38 {
                &mut self.fg
            } else {
                &mut self.bg
            };
            *plane = Some(color);
        }
    }
    /// Ordered inline CSS without a trailing separator.
    fn css(&self) -> String {
        let mut parts = Vec::new();
        if let Some(color) = &self.fg {
            parts.push(format!("color:{color}"));
        }
        if let Some(color) = &self.bg {
            parts.push(format!("background-color:{color}"));
        }
        for (active, css) in [
            (self.intensity.bold, "font-weight:bold"),
            (self.intensity.dim, "opacity:0.6"),
            (self.italic, "font-style:italic"),
            (self.underline, "text-decoration:underline"),
        ] {
            if active {
                parts.push(css.to_owned());
            }
        }
        parts.join(";")
    }
}

/// Visit plain text and recognized ASCII SGR parameters in source order.
pub(crate) fn segments(text: &str, mut visit: impl FnMut(&str, Option<&str>)) {
    let mut rest = text;
    let mut search = 0;
    while let Some(start) = rest[search..].find("\x1b[").map(|i| i + search) {
        let params = &rest[start + 2..];
        let end = params
            .bytes()
            .take_while(|b| b.is_ascii_digit() || *b == b';')
            .count();
        if params.as_bytes().get(end) == Some(&b'm') {
            visit(&rest[..start], Some(&params[..end]));
            rest = &params[end + 1..];
            search = 0;
        } else {
            search = start + 2;
        }
    }
    visit(rest, None);
}

/// Convert terminal text to literal escaped HTML with inline styles.
#[must_use]
pub fn ansi_to_html(text: &str) -> String {
    let mut style = TextStyle::default();
    let mut out = String::new();
    let mut in_span = false;
    segments(text, |plain, params| {
        escape_into(plain, &mut out, EscapeMode::HtmlDecimalPadded);
        if let Some(params) = params {
            if in_span {
                out.push_str("</span>");
            }
            style.apply(params);
            let css = style.css();
            in_span = !css.is_empty();
            if in_span {
                out.push_str("<span style=\"");
                out.push_str(&css);
                out.push_str("\">");
            }
        }
    });
    if in_span {
        out.push_str("</span>");
    }
    out
}

/// Wrap independently converted lines without inserting source whitespace.
#[must_use]
pub fn ansi_lines_to_html(lines: &[String]) -> String {
    let mut out = String::new();
    for line in lines {
        let html = ansi_to_html(line);
        out.push_str("<div class=\"ansi-line\">");
        out.push_str(if html.is_empty() { "&nbsp;" } else { &html });
        out.push_str("</div>");
    }
    out
}

/// Consume only complete extended-color groups, retaining remaining standalone codes.
fn extended_color(codes: &mut std::slice::Iter<'_, u32>) -> Option<String> {
    let (color, consumed) = match codes.as_slice() {
        [5, index, ..] => (ansi256_to_hex(*index), 2),
        [2, r, g, b, ..] => (format!("rgb({r},{g},{b})"), 4),
        _ => return None,
    };
    codes.nth(consumed - 1);
    Some(color)
}
