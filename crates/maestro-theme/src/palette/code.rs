//! Terminal rows and escaped HTML fragments from highlighter spans.
use super::styles::paint;
use super::syntax_labels;
use super::{LiveTheme, SyntaxHighlighter, SyntaxSpan, ThemeColor, ThemeError};
use std::error::Error;

/// What a failing engine returns instead of rows.
pub(super) type Recovery = fn(&LiveTheme, &str) -> Vec<String>;

/// The label, when it is present, nonempty and a language the syntax registry admits.
///
/// A label the registry does not name is painted as code, never handed to another grammar.
fn admitted(lang: Option<&str>) -> Option<&str> {
    lang.filter(|lang| !lang.is_empty() && syntax_labels::admits(lang))
}

/// Split `code` at every line feed, coloring each row as unclassified code.
///
/// # Errors
/// Returns the failure to read the published theme.
fn colored_rows(theme: &LiveTheme, code: &str) -> Result<Vec<String>, ThemeError> {
    let theme = theme.get()?;
    Ok(code
        .split('\n')
        .map(|row| paint(Some(&*theme), &ThemeColor::MdCodeBlock, row))
        .collect())
}

/// `colored_rows` for a caller that cannot report a failure: unstyled rows instead.
pub(super) fn colored_rows_or_raw(theme: &LiveTheme, code: &str) -> Vec<String> {
    colored_rows(theme, code).unwrap_or_else(|_| raw_rows(theme, code))
}

/// Split `code` at every line feed without styling.
fn raw_rows(_theme: &LiveTheme, code: &str) -> Vec<String> {
    code.split('\n').map(str::to_owned).collect()
}

/// Rows of an admitted `lang`: the engine's rows when it has that grammar, code rows when it
/// does not, and `recover` when the engine fails.
fn admitted_rows(
    theme: &LiveTheme,
    highlighter: &dyn SyntaxHighlighter,
    code: &str,
    lang: &str,
    recover: Recovery,
) -> Vec<String> {
    if !highlighter.supports_language(lang) {
        return colored_rows_or_raw(theme, code);
    }
    match highlighter.highlight(code, lang) {
        Ok(spans) => styled_rows(theme, code, &spans),
        Err(_) => recover(theme, code),
    }
}

/// Rows of highlighted code; `recover` supplies the rows when the engine fails.
///
/// A missing, empty or unadmitted label skips the engine and colors the rows as code, which
/// stays unstyled when no theme is published; text the engine leaves unclassified is
/// returned unchanged.
pub(super) fn highlight_rows(
    theme: &LiveTheme,
    highlighter: &dyn SyntaxHighlighter,
    (code, lang): (&str, Option<&str>),
    recover: Recovery,
) -> Vec<String> {
    let Some(lang) = admitted(lang) else {
        return colored_rows_or_raw(theme, code);
    };
    admitted_rows(theme, highlighter, code, lang, recover)
}

/// Rows of `code` split at line feeds; classified runs are colored from the live theme and
/// unclassified runs stay unchanged.
///
/// A classified run that spans line breaks is colored once per nonempty row piece.
fn styled_rows(theme: &LiveTheme, code: &str, spans: &[SyntaxSpan]) -> Vec<String> {
    let (mut rows, mut row) = (Vec::new(), String::new());
    for span in spans {
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
                match span.color.as_ref() {
                    Some(key) => row.push_str(&paint(theme.get().ok().as_deref(), key, piece)),
                    None => row.push_str(piece),
                }
            }
        }
    }
    rows.push(row);
    rows
}

/// Highlight `code` for a terminal, never guessing a language.
///
/// A missing, empty or unadmitted label colors every row as code without calling the
/// engine, text the engine leaves unclassified stays unchanged, and an engine failure
/// returns the rows unstyled.
///
/// # Errors
/// Returns the failure to read the published theme for a label the syntax registry does
/// not admit, whose rows are drawn in the code color.
pub fn highlight_code(
    theme: &LiveTheme,
    highlighter: &dyn SyntaxHighlighter,
    code: &str,
    lang: Option<&str>,
) -> Result<Vec<String>, ThemeError> {
    let Some(lang) = admitted(lang) else {
        return colored_rows(theme, code);
    };
    Ok(admitted_rows(theme, highlighter, code, lang, raw_rows))
}

/// The label, when it is present, nonempty and a language the engine has a grammar for.
fn engine_language<'a>(
    highlighter: &dyn SyntaxHighlighter,
    lang: Option<&'a str>,
) -> Option<&'a str> {
    lang.filter(|lang| !lang.is_empty() && highlighter.supports_language(lang))
}

/// Escape `text` for HTML content and attribute values.
fn escape(text: &str, out: &mut String) {
    maestro_text::markup::escape_into(text, out, maestro_text::markup::EscapeMode::HtmlHex);
}

/// Highlight `code` as an escaped HTML fragment of `hljs-*` token spans.
///
/// A missing, empty or unsupported label returns `None` without calling the engine; the
/// caller owns any automatic recognition and plain fallback.
///
/// # Errors
/// Returns the engine's failure for a label it supports.
pub fn highlight_html(
    highlighter: &dyn SyntaxHighlighter,
    code: &str,
    lang: Option<&str>,
) -> Result<Option<String>, Box<dyn Error + Send + Sync>> {
    let Some(lang) = engine_language(highlighter, lang) else {
        return Ok(None);
    };
    render_html(highlighter, code, lang).map(Some)
}

/// Paint original text through the shared span-to-HTML emitter.
fn render_html(
    highlighter: &dyn SyntaxHighlighter,
    code: &str,
    lang: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let mut html = String::new();
    for span in highlighter.highlight(code, lang)? {
        let text = code.get(span.range).unwrap_or_default();
        let class = span.color.as_ref().and_then(|c| syntax_class(c.as_str()));
        match class {
            Some(name) => {
                html.push_str("<span class=\"hljs-");
                html.push_str(&name);
                html.push_str("\">");
                escape(text, &mut html);
                html.push_str("</span>");
            }
            None => escape(text, &mut html),
        }
    }
    Ok(html)
}

/// Highlight an HTML code block using its surface's registered language policy.
///
/// Admitted labels use their canonical grammar; missing or unregistered labels
/// select automatically. A plain result or unavailable native syntax returns
/// escaped original text. Terminal and explicit HTML highlighting are unchanged.
///
/// # Errors
/// Returns grammar preparation failures or the painter's original failure.
pub fn highlight_auto(
    highlighter: &dyn SyntaxHighlighter,
    code: &str,
    lang: Option<&str>,
    profile: super::HighlightProfile,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let language = crate::highlight::select(code, lang, profile)?;
    if let Some(language) = language.filter(|language| highlighter.supports_language(language)) {
        render_html(highlighter, code, language)
    } else {
        let mut html = String::new();
        escape(code, &mut html);
        Ok(html)
    }
}

/// The class spelling emitted by both syntax projections.
pub(crate) fn syntax_class(key: &str) -> Option<String> {
    key.strip_prefix("syntax").map(str::to_ascii_lowercase)
}

/// CSS for emitted syntax classes using selected-pack custom properties.
/// # Errors
/// Returns an invalid shipped role-table error.
pub fn highlight_css() -> Result<String, super::ThemeError> {
    super::brand::syntax_css()
}
