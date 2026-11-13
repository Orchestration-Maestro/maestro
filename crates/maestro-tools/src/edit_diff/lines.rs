//! Line-by-line rebuilding of an edited text, so a line no match reaches keeps its bytes.

use super::Matched;
use crate::utils::diagnostics::failure;
use maestro_models::DiagnosticErrorInfo;

/// One line of a text and the line break kept after it.
pub(super) struct Line<'a> {
    /// The line without its break.
    text: &'a str,
    /// The break as it stands in the text; empty after the last line.
    end: &'a str,
}

/// Split LF-normalized `content` at each LF, keeping every break with its line.
pub(super) fn lf_lines(content: &str) -> Vec<Line<'_>> {
    let mut lines: Vec<_> = content
        .split('\n')
        .map(|text| Line { text, end: "\n" })
        .collect();
    if let Some(last) = lines.last_mut() {
        last.end = "";
    }
    lines
}

/// Rebuild a text line by line from `base`, its matching view with one LF per line break.
///
/// A line no match reaches keeps its own text and break. Every other line takes the text
/// of `base` with its matches replaced, and keeps its own break unless a match covers it.
///
/// # Errors
/// Returns a diagnostic when `base` and `lines` differ in line count, so no line is dropped.
pub(super) fn splice(
    lines: &[Line],
    base: &str,
    matched: &[Matched],
    path: &str,
) -> Result<String, DiagnosticErrorInfo> {
    if base.split('\n').count() != lines.len() {
        return Err(failure(&format!(
            "Cannot edit {path}: its matching view and its content differ in line count."
        )));
    }
    let mut out = String::with_capacity(base.len());
    let mut pending = matched.iter().peekable();
    let (mut start, mut done) = (0, 0);
    for (line, kept) in lines.iter().zip(base.split('\n')) {
        let text_end = start + kept.len();
        let next = text_end + line.end.len().min(1);
        if done <= start && pending.peek().is_none_or(|entry| entry.index >= next) {
            out.push_str(line.text);
            out.push_str(line.end);
            done = next;
        } else {
            while let Some(entry) = pending.next_if(|entry| entry.index < next) {
                out.push_str(&base[done..entry.index]);
                out.push_str(entry.new);
                done = entry.index + entry.length;
            }
            if done <= text_end {
                out.push_str(&base[done..text_end]);
                out.push_str(line.end);
                done = next;
            }
        }
        start = next;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{lf_lines, splice};

    #[test]
    fn a_view_with_another_line_count_is_refused_before_any_line_is_dropped() {
        let error = splice(&lf_lines("one\ntwo\n"), "one two\n", &[], "note.txt").unwrap_err();
        assert!(error.message.contains("note.txt"), "{}", error.message);
        assert_eq!(
            splice(&lf_lines("one\ntwo\n"), "one\ntwo\n", &[], "note.txt").unwrap(),
            "one\ntwo\n"
        );
    }
}
