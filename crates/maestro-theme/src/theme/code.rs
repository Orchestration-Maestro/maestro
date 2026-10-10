//! Terminal rows and escaped HTML fragments from highlighter spans.
use super::styles::paint;
use super::{LiveTheme, SyntaxHighlighter, SyntaxSpan, ThemeColor};
use std::error::Error;

/// What a failing engine returns instead of rows.
pub(super) type Recovery = fn(&LiveTheme, &str) -> Vec<String>;

/// The label, when it is present, nonempty and supported.
fn supported<'a>(highlighter: &dyn SyntaxHighlighter, lang: Option<&'a str>) -> Option<&'a str> {
    lang.filter(|lang| !lang.is_empty() && highlighter.supports_language(lang))
}

/// Split `code` at every line feed, coloring each row as unclassified code.
pub(super) fn colored_rows(theme: &LiveTheme, code: &str) -> Vec<String> {
    code.split('\n')
        .map(|row| paint(theme, &ThemeColor::MdCodeBlock, row))
        .collect()
}

/// Split `code` at every line feed without styling.
fn raw_rows(_theme: &LiveTheme, code: &str) -> Vec<String> {
    code.split('\n').map(str::to_owned).collect()
}

/// Rows of highlighted code; `recover` supplies the rows when the engine fails.
///
/// A missing, empty or unsupported label skips the engine and colors the rows as code.
pub(super) fn highlight_rows(
    theme: &LiveTheme,
    highlighter: &dyn SyntaxHighlighter,
    (code, lang): (&str, Option<&str>),
    recover: Recovery,
) -> Vec<String> {
    let Some(lang) = supported(highlighter, lang) else {
        return colored_rows(theme, code);
    };
    match highlighter.highlight(code, lang) {
        Ok(spans) => styled_rows(theme, code, &spans),
        Err(_) => recover(theme, code),
    }
}

/// Rows of `code` split at line feeds, each run colored from the live theme.
///
/// A run that spans line breaks is colored once per row.
fn styled_rows(theme: &LiveTheme, code: &str, spans: &[SyntaxSpan]) -> Vec<String> {
    let (mut rows, mut row) = (Vec::new(), String::new());
    for span in spans {
        let key = span.color.clone().unwrap_or(ThemeColor::MdCodeBlock);
        for (index, piece) in code
            .get(span.range.clone())
            .unwrap_or_default()
            .split('\n')
            .enumerate()
        {
            if index > 0 {
                rows.push(std::mem::take(&mut row));
            }
            if !piece.is_empty() {
                row.push_str(&paint(theme, &key, piece));
            }
        }
    }
    rows.push(row);
    rows
}

/// Highlight `code` for a terminal, never guessing a language.
///
/// A missing, empty or unsupported label colors every row as code without calling the
/// engine, and an engine failure returns the rows unstyled.
#[must_use]
pub fn highlight_code(
    theme: &LiveTheme,
    highlighter: &dyn SyntaxHighlighter,
    code: &str,
    lang: Option<&str>,
) -> Vec<String> {
    highlight_rows(theme, highlighter, (code, lang), raw_rows)
}

/// Escape `text` for HTML content and attribute values.
fn escape(text: &str, out: &mut String) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            c => out.push(c),
        }
    }
}

/// Highlight `code` as an escaped HTML fragment of `hljs-*` token spans.
///
/// A missing, empty or unsupported label returns `None` without calling the engine; the
/// caller owns any automatic recognition and plain fallback.
///
/// # Errors
/// Returns the engine's failure for a supported label.
pub fn highlight_html(
    highlighter: &dyn SyntaxHighlighter,
    code: &str,
    lang: Option<&str>,
) -> Result<Option<String>, Box<dyn Error + Send + Sync>> {
    let Some(lang) = supported(highlighter, lang) else {
        return Ok(None);
    };
    let mut html = String::new();
    for span in highlighter.highlight(code, lang)? {
        let text = code.get(span.range).unwrap_or_default();
        let class = span
            .color
            .as_ref()
            .and_then(|c| c.as_str().strip_prefix("syntax"));
        match class {
            Some(name) => {
                html.push_str("<span class=\"hljs-");
                html.push_str(&name.to_ascii_lowercase());
                html.push_str("\">");
                escape(text, &mut html);
                html.push_str("</span>");
            }
            None => escape(text, &mut html),
        }
    }
    Ok(Some(html))
}
