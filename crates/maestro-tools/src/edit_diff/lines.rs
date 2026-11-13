//! Line-by-line rebuilding of an edited text, so a line no match reaches keeps its bytes.

use super::{LineEnding, Matched, restore_line_endings};
use crate::utils::diagnostics::failure;
use maestro_models::DiagnosticErrorInfo;

/// One line of a text and the line break kept after it.
pub(super) struct Line<'a> {
    /// The line without its break.
    text: &'a str,
    /// The break as it stands in the text; empty after the last line.
    end: &'a str,
}

/// Split `content` at each of `breaks`, keeping every break with its line; CR LF is one break.
pub(super) fn split_lines<'a>(content: &'a str, breaks: &[char]) -> Vec<Line<'a>> {
    let mut lines = Vec::new();
    let mut rest = content;
    while let Some(at) = rest.find(breaks) {
        let width = if rest[at..].starts_with("\r\n") { 2 } else { 1 };
        lines.push(Line {
            text: &rest[..at],
            end: &rest[at..at + width],
        });
        rest = &rest[at + width..];
    }
    lines.push(Line {
        text: rest,
        end: "",
    });
    lines
}

/// The rebuilt text, and whether a lone CR break is its last piece.
#[derive(Default)]
struct Rebuilt {
    /// The text so far.
    text: String,
    /// Whether the text ends in a lone CR that is a kept line break.
    lone_cr: bool,
}

impl Rebuilt {
    /// Append `piece`. A kept lone CR that `piece` would join into one CR LF becomes LF
    /// first, so the two line breaks stay two.
    fn push(&mut self, piece: &str) {
        if piece.is_empty() {
            return;
        }
        if self.lone_cr && piece.starts_with('\n') {
            self.text.pop();
            self.text.push('\n');
        }
        self.text.push_str(piece);
        self.lone_cr = false;
    }

    /// Append a line's own break.
    fn end_line(&mut self, end: &str) {
        self.push(end);
        self.lone_cr = end == "\r";
    }
}

/// Rebuild a text line by line from `base`, its matching view with one LF per line break.
///
/// A line no match reaches keeps its own text and break. Every other line takes the text
/// of `base` with its matches replaced, and keeps its own break unless a match covers it.
/// Replacement text takes `ending`.
///
/// # Errors
/// Returns a diagnostic when `base` and `lines` differ in line count, so no line is dropped.
pub(super) fn splice(
    lines: &[Line],
    base: &str,
    matched: &[Matched],
    ending: LineEnding,
    path: &str,
) -> Result<String, DiagnosticErrorInfo> {
    if base.split('\n').count() != lines.len() {
        return Err(failure(&format!(
            "Cannot edit {path}: its matching view and its content differ in line count."
        )));
    }
    let mut out = Rebuilt::default();
    let mut pending = matched.iter().peekable();
    let (mut start, mut done) = (0, 0);
    for (line, kept) in lines.iter().zip(base.split('\n')) {
        let text_end = start + kept.len();
        let next = text_end + line.end.len().min(1);
        if done <= start && pending.peek().is_none_or(|entry| entry.index >= next) {
            out.push(line.text);
            out.end_line(line.end);
            done = next;
        } else {
            while let Some(entry) = pending.next_if(|entry| entry.index < next) {
                out.push(&base[done..entry.index]);
                out.push(&restore_line_endings(entry.new, ending));
                done = entry.index + entry.length;
            }
            if done <= text_end {
                out.push(&base[done..text_end]);
                out.end_line(line.end);
                done = next;
            }
        }
        start = next;
    }
    Ok(out.text)
}

#[cfg(test)]
mod tests {
    use super::{LineEnding, splice, split_lines};

    #[test]
    fn a_view_with_another_line_count_is_refused_before_any_line_is_dropped() {
        let content = split_lines("one\ntwo\n", &['\n']);
        let error = splice(&content, "one two\n", &[], LineEnding::Lf, "note.txt").unwrap_err();
        assert!(error.message.contains("note.txt"), "{}", error.message);
        assert_eq!(
            splice(&content, "one\ntwo\n", &[], LineEnding::Lf, "note.txt").unwrap(),
            "one\ntwo\n"
        );
    }
}
