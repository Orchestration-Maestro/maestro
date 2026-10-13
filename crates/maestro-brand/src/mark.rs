//! Mark substitution over the original template only.
use crate::{BrandError, BrandPresentation, role_color};

/// The title reference.
const WORDMARK: &str = "{{wordmark}}";
/// The start of a color reference.
const COLOR_START: &str = "var(--";
/// The single color of the monochrome variant.
const CURRENT_COLOR: &str = "currentColor";

impl BrandPresentation<'_> {
    /// Substitute wordmark, color roles and currentColor in the original template once.
    ///
    /// The wordmark is XML-escaped; inserted values are not scanned again.
    ///
    /// # Errors
    /// Returns a missing-color-role error or an unterminated-color error for a reached token.
    pub fn fill_mark(&self, template: &str) -> Result<String, BrandError> {
        let mut out = String::with_capacity(template.len());
        let mut rest = template;
        while let Some(at) = rest.find(['{', 'v', 'c']) {
            out.push_str(&rest[..at]);
            let tail = &rest[at..];
            rest = &tail[reference(tail, self, &mut out)?..];
        }
        out.push_str(rest);
        Ok(out)
    }
}

/// Append what the text at the start of `tail` stands for; returns the bytes it spans.
fn reference(
    tail: &str,
    presentation: &BrandPresentation<'_>,
    out: &mut String,
) -> Result<usize, BrandError> {
    if tail.starts_with(WORDMARK) {
        escape_text(out, presentation.wordmark);
        return Ok(WORDMARK.len());
    }
    if let Some(after) = tail.strip_prefix(COLOR_START) {
        let end = after.find(')').ok_or(BrandError::UnterminatedColor)?;
        out.push_str(role_color(&presentation.colors, &after[..end])?);
        return Ok(COLOR_START.len() + end + 1);
    }
    if tail.starts_with(CURRENT_COLOR) {
        out.push_str(role_color(&presentation.colors, "text")?);
        return Ok(CURRENT_COLOR.len());
    }
    let first = tail.chars().next().map_or(0, char::len_utf8);
    out.push_str(&tail[..first]);
    Ok(first)
}

/// Append `text` with the five XML special characters escaped.
fn escape_text(out: &mut String, text: &str) {
    maestro_text::markup::escape_into(text, out, maestro_text::markup::EscapeMode::Xml);
}
