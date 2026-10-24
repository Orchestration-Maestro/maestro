//! Changed-span decoration outside recognized terminal escape boundaries.

use super::{sequence::Kind, word::Run};
use maestro_text::is_whitespace_scalar;
use maestro_theme::Theme;
use maestro_tui::extract_ansi_code;
use std::ops::Range;

/// One output side before styling.
#[derive(Default)]
struct Side {
    /// Composed text, including selected whitespace.
    text: String,
    /// Changed byte intervals.
    changes: Vec<Range<usize>>,
    /// Whether a changed run has already been appended.
    changed: bool,
}
impl Side {
    /// Append a run, excluding first-change indentation from its inverse span.
    fn push(&mut self, value: &str, changed: bool) {
        let start = self.text.len();
        self.text.push_str(value);
        if changed {
            let leading = if self.changed {
                0
            } else {
                value.len() - value.trim_start_matches(is_whitespace_scalar).len()
            };
            self.changed = true;
            self.changes.push(start + leading..self.text.len());
        }
    }
    /// Adjust intervals in one monotone scan of the composed text.
    fn adjust(&mut self) {
        let (mut at, mut range) = (0, 0);
        let (mut failed_csi, mut failed_string) = (false, false);
        while at < self.text.len() {
            let category = self.text.as_bytes().get(at + 1).copied();
            let failed = match category {
                Some(b'[') => failed_csi,
                Some(b']' | b'_') => failed_string,
                _ => false,
            };
            if !failed && let Some(code) = extract_ansi_code(&self.text, at) {
                let end = at + code.length;
                range = self.adjust_ranges(range, at..end);
                at = end;
                continue;
            }
            failed_csi |= self.text[at..].starts_with("\x1b[");
            failed_string |=
                self.text[at..].starts_with("\x1b]") || self.text[at..].starts_with("\x1b_");
            at += self.text[at..].chars().next().map_or(1, char::len_utf8);
        }
    }
    /// Move boundaries strictly inside one escape out to its edges.
    fn adjust_ranges(&mut self, mut range: usize, escape: Range<usize>) -> usize {
        while range < self.changes.len() && self.changes[range].end <= escape.start {
            range += 1;
        }
        for change in self.changes[range..]
            .iter_mut()
            .take_while(|change| change.start < escape.end)
        {
            if change.start > escape.start {
                change.start = escape.end;
            }
            if change.end < escape.end {
                change.end = escape.start;
            }
        }
        range
    }
    /// Emit inverse spans with the shared theme decorator.
    fn render(mut self) -> String {
        self.adjust();
        let mut output = String::new();
        let mut at = 0;
        for range in self
            .changes
            .into_iter()
            .filter(|range| range.start < range.end)
        {
            output.push_str(&self.text[at..range.start]);
            output.push_str(&Theme::inverse(&self.text[range.clone()], true));
            at = range.end;
        }
        output.push_str(&self.text[at..]);
        output
    }
}
/// Render both sides of the selected word runs.
pub(super) fn render(runs: &[Run]) -> (String, String) {
    let (mut old, mut new) = (Side::default(), Side::default());
    for run in runs {
        match run.kind {
            Kind::Removed => old.push(&run.value, true),
            Kind::Added => new.push(&run.value, true),
            Kind::Unchanged => {
                old.push(&run.value, false);
                new.push(&run.value, false);
            }
        }
    }
    (old.render(), new.render())
}
