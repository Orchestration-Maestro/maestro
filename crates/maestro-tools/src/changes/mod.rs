//! Numbered edit diff rendering with the published theme.

pub mod text;

mod inverse;
pub(crate) mod sequence;
mod whitespace;
mod word;

use crate::output::render_utils::replace_tabs;
use maestro_text::is_whitespace_scalar;
use maestro_theme::{LiveTheme, ThemeColor, ThemeError};

/// Optional diff rendering arguments.
#[derive(Clone, Debug, Default)]
pub struct RenderDiffOptions {
    /// Accepted file path; it does not select syntax highlighting.
    pub file_path: Option<String>,
}

/// Color a numbered edit diff with the currently published theme.
///
/// # Errors
/// Propagates errors from [`LiveTheme::get`] and the selected foreground colors.
pub fn render_diff(
    diff_text: &str,
    _options: &RenderDiffOptions,
    theme: &LiveTheme,
) -> Result<String, ThemeError> {
    let theme = theme.get()?;
    let mut rows = diff_text
        .split('\n')
        .map(|line| (line, parse_line(line)))
        .peekable();
    let mut output = Vec::new();
    while let Some((raw, parsed)) = rows.next() {
        let Some(row) = parsed else {
            output.push(theme.fg(&ThemeColor::ToolDiffContext, raw)?);
            continue;
        };
        if row.prefix != '-' {
            output.push(theme.fg(&row.color(), &row.display(&replace_tabs(row.content)))?);
            continue;
        }
        let mut removed = vec![row];
        while rows
            .peek()
            .is_some_and(|(_, row)| row.as_ref().is_some_and(|row| row.prefix == '-'))
        {
            if let Some((_, Some(row))) = rows.next() {
                removed.push(row);
            }
        }
        let mut added = Vec::new();
        while rows
            .peek()
            .is_some_and(|(_, row)| row.as_ref().is_some_and(|row| row.prefix == '+'))
        {
            if let Some((_, Some(row))) = rows.next() {
                added.push(row);
            }
        }
        if removed.len() == 1 && added.len() == 1 {
            let runs = word::diff_words(
                &replace_tabs(removed[0].content),
                &replace_tabs(added[0].content),
            );
            let (old, new) = inverse::render(&runs);
            output.push(theme.fg(&removed[0].color(), &removed[0].display(&old))?);
            output.push(theme.fg(&added[0].color(), &added[0].display(&new))?);
        } else {
            for row in removed.into_iter().chain(added) {
                output.push(theme.fg(&row.color(), &row.display(&replace_tabs(row.content)))?);
            }
        }
    }
    Ok(output.join("\n"))
}

/// Borrowed fields of a recognized numbered row.
struct Row<'a> {
    /// Display sign.
    prefix: char,
    /// Number and its leading spacing.
    number: &'a str,
    /// Display content.
    content: &'a str,
}
impl Row<'_> {
    /// Foreground selected by this row.
    fn color(&self) -> ThemeColor {
        match self.prefix {
            '-' => ThemeColor::ToolDiffRemoved,
            '+' => ThemeColor::ToolDiffAdded,
            _ => ThemeColor::ToolDiffContext,
        }
    }
    /// Numbered template with supplied content.
    fn display(&self, content: &str) -> String {
        let prefix = if matches!(self.prefix, '+' | '-') {
            self.prefix
        } else {
            ' '
        };
        format!("{prefix}{} {content}", self.number)
    }
}
/// Recognize the display grammar without normalizing the captured fields.
fn parse_line(line: &str) -> Option<Row<'_>> {
    let prefix = line.chars().next()?;
    if !matches!(prefix, '+' | '-') && !is_whitespace_scalar(prefix) {
        return None;
    }
    let start = prefix.len_utf8();
    let rest = &line[start..];
    let whitespace = rest.len() - rest.trim_start_matches(is_whitespace_scalar).len();
    let digits = rest[whitespace..]
        .bytes()
        .take_while(u8::is_ascii_digit)
        .count();
    let separator = if digits > 0 {
        whitespace + digits
    } else {
        let (index, _) = rest[..whitespace].char_indices().next_back()?;
        index
    };
    let scalar = rest[separator..].chars().next()?;
    if !is_whitespace_scalar(scalar) {
        return None;
    }
    let content = &rest[separator + scalar.len_utf8()..];
    if content.contains(['\r', '\n', '\u{2028}', '\u{2029}']) {
        return None;
    }
    Some(Row {
        prefix,
        number: &rest[..separator],
        content,
    })
}
