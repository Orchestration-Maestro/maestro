//! Mark templates: one pass over the original text, substituting its color and title references.
use super::projection::{BrandPresentation, role_color};
use super::{BrandMode, BrandPack};
use crate::theme::{ThemeError, ThemeOperations};

/// The title reference.
const WORDMARK: &str = "{{wordmark}}";
/// The start of a color reference.
const COLOR_START: &str = "var(--";
/// The single color of the monochrome variant.
const CURRENT_COLOR: &str = "currentColor";

impl BrandPack {
    /// Read the default template (`None`) or the named variant and fill in `mode`'s colors.
    ///
    /// The wordmark is XML-escaped into `{{wordmark}}`, `var(--role)` becomes that role's color
    /// and `currentColor` becomes the `text` color. Inserted text is not scanned again.
    ///
    /// # Errors
    /// Returns `Unknown brand mark variant` before any read, the presentation failures, the
    /// template read failure, `Missing brand color role` or `Unterminated brand template color`.
    pub fn mark_svg(
        &self,
        mode: BrandMode,
        variant: Option<&str>,
        operations: &dyn ThemeOperations,
    ) -> Result<String, ThemeError> {
        let mark = &self.data.mark;
        let path = match variant {
            None => &mark.template,
            Some(name) => mark.variants.get(name).ok_or_else(|| {
                ThemeError::message(format!("Unknown brand mark variant: {name}"))
            })?,
        };
        let presentation = self.presentation(mode)?;
        let template = operations.read_to_string(path).map_err(ThemeError::io)?;
        let mut out = String::with_capacity(template.len());
        let mut rest = template.as_str();
        while let Some(at) = rest.find(['{', 'v', 'c']) {
            out.push_str(&rest[..at]);
            let tail = &rest[at..];
            rest = &tail[reference(tail, &presentation, &mut out)?..];
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
) -> Result<usize, ThemeError> {
    if tail.starts_with(WORDMARK) {
        escape_text(out, presentation.wordmark);
        return Ok(WORDMARK.len());
    }
    if let Some(after) = tail.strip_prefix(COLOR_START) {
        let end = after
            .find(')')
            .ok_or_else(|| ThemeError::message("Unterminated brand template color".to_owned()))?;
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
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
}
