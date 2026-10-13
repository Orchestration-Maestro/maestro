//! Numbered line diffs with surrounding context.

use super::text_diff::{Change, diff_lines};

/// A numbered diff and the first changed line of the new text.
#[derive(Debug, PartialEq, Eq)]
pub struct EditDiffResult {
    /// The numbered diff.
    pub diff: String,
    /// The first changed line in the new text.
    pub first_changed_line: Option<usize>,
}

/// The numbered output being written.
struct Numbered {
    /// Output lines.
    output: Vec<String>,
    /// Number column width.
    width: usize,
    /// Next old line number.
    old: usize,
    /// Next new line number.
    new: usize,
}

impl Numbered {
    /// Write `line` with `sign` and `number`.
    fn push(&mut self, sign: char, number: usize, line: &str) {
        let width = self.width;
        self.output.push(format!("{sign}{number:>width$} {line}"));
    }

    /// One unchanged line, numbered in the old text.
    fn context(&mut self, line: &str) {
        self.push(' ', self.old, line);
        self.old += 1;
        self.new += 1;
    }

    /// The lines of one removed or added run.
    fn change(&mut self, change: Change, lines: &[&str]) {
        for line in lines {
            if let Change::Added(_) = change {
                self.push('+', self.new, line);
                self.new += 1;
            } else {
                self.push('-', self.old, line);
                self.old += 1;
            }
        }
    }

    /// Show `lines` of one unchanged run around its neighbouring changes.
    fn unchanged(&mut self, lines: &[&str], (leading, trailing): (bool, bool), context: usize) {
        let len = lines.len();
        let (head, tail) = match (leading, trailing) {
            (true, true) if len <= context.saturating_mul(2) => (len, 0),
            (true, true) => (context, context),
            (true, false) => (context.min(len), 0),
            (false, true) => (0, context.min(len)),
            (false, false) => (0, 0),
        };
        let skipped = len - head - tail;
        lines[..head].iter().for_each(|line| self.context(line));
        if skipped > 0 && (leading || trailing) {
            self.output
                .push(format!(" {:width$} ...", "", width = self.width));
        }
        self.old += skipped;
        self.new += skipped;
        lines[len - tail..]
            .iter()
            .for_each(|line| self.context(line));
    }
}

/// The lines of one change value, without the empty piece after a final LF.
fn lines_of(value: &str) -> Vec<&str> {
    value
        .strip_suffix('\n')
        .unwrap_or(value)
        .split('\n')
        .collect()
}

/// A numbered line diff of two texts.
///
/// Omitted context selects four lines; explicit context includes zero.
#[must_use]
pub fn generate_diff_string(
    old_content: &str,
    new_content: &str,
    context_lines: Option<usize>,
) -> EditDiffResult {
    let parts = diff_lines(old_content, new_content);
    let rows = old_content
        .split('\n')
        .count()
        .max(new_content.split('\n').count());
    let mut numbered = Numbered {
        output: Vec::new(),
        width: rows.to_string().len(),
        old: 1,
        new: 1,
    };
    let mut first_changed_line = None;
    let mut last_was_change = false;
    for (index, &part) in parts.iter().enumerate() {
        let (Change::Added(value) | Change::Removed(value) | Change::Unchanged(value)) = part;
        let lines = lines_of(value);
        if let Change::Unchanged(_) = part {
            let next_is_change = parts
                .get(index + 1)
                .is_some_and(|next| !matches!(next, Change::Unchanged(_)));
            numbered.unchanged(
                &lines,
                (last_was_change, next_is_change),
                context_lines.unwrap_or(4),
            );
        } else {
            first_changed_line.get_or_insert(numbered.new);
            numbered.change(part, &lines);
        }
        last_was_change = !matches!(part, Change::Unchanged(_));
    }
    EditDiffResult {
        diff: numbered.output.join("\n"),
        first_changed_line,
    }
}
