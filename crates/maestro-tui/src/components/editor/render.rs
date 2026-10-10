//! Logical-line layout, vertical viewport and display-only cursor decoration.
use super::{Editor, markers, word_wrap_line};
use crate::components::cursor::display_cursor;
use crate::{CURSOR_MARKER, truncate_to_width, visible_width};
use std::borrow::Cow;
/// A rendered logical chunk with an optional local byte cursor.
struct LayoutLine {
    /// Original text selected by wrapping.
    text: String,
    /// Byte cursor owned by this chunk.
    cursor: Option<usize>,
}
impl Editor {
    /// Composes border styling before reading the buffer for layout.
    pub(super) fn draw(&self, width: usize) -> Vec<String> {
        let padding = super::normalize(self.padding.get(), 0, width.saturating_sub(1) / 2, 0);
        let content = width.saturating_sub(2 * padding).max(1);
        let layout_width = content.saturating_sub(usize::from(padding == 0)).max(1);
        self.width.set(layout_width);
        let horizontal = (self.border_color())("─");
        let lines = self.layout(layout_width);
        let maximum = visible_lines(self.terminal.borrow().rows());
        let cursor = lines
            .iter()
            .position(|line| line.cursor.is_some())
            .unwrap_or(0);
        let mut scroll = self.scroll.get();
        if cursor < scroll {
            scroll = cursor;
        } else if cursor >= scroll + maximum {
            scroll = cursor - maximum + 1;
        }
        scroll = scroll.min(lines.len().saturating_sub(maximum));
        self.scroll.set(scroll);
        let end = (scroll + maximum).min(lines.len());
        let mut result = vec![self.border(width, scroll, "↑", &horizontal)];
        result.extend(
            lines[scroll..end]
                .iter()
                .map(|line| self.row(line, width, padding)),
        );
        result.push(self.border(width, lines.len() - end, "↓", &horizontal));
        result
    }
    /// Wraps each logical line and assigns boundary cursors to the following chunk.
    fn layout(&self, width: usize) -> Vec<LayoutLine> {
        let state = self.state.borrow();
        let mut lines = Vec::new();
        for (index, text) in state.current.lines.iter().enumerate() {
            let segments = markers::segments(text, state.pastes.len());
            let chunks = word_wrap_line(text, width, Some(&segments));
            let last = chunks.len() - 1;
            for (part, chunk) in chunks.into_iter().enumerate() {
                let cursor = state.current.cursor;
                let owns = index == cursor.line
                    && cursor.col >= chunk.start_index
                    && (part == last || cursor.col < chunk.end_index);
                lines.push(LayoutLine {
                    cursor: owns.then(|| (cursor.col - chunk.start_index).min(chunk.text.len())),
                    text: chunk.text,
                });
            }
        }
        lines
    }
    /// Clips scroll metadata before styling, otherwise repeats the once-styled glyph.
    fn border(&self, width: usize, count: usize, direction: &str, horizontal: &str) -> String {
        if count == 0 {
            return horizontal.repeat(width);
        }
        let indicator = format!("─── {direction} {count} more ");
        let cells = visible_width(&indicator);
        let text = if cells <= width {
            format!("{indicator}{}", "─".repeat(width - cells))
        } else {
            truncate_to_width(&indicator, width, crate::TruncateOptions::default())
        };
        (self.border_color())(&text)
    }
    /// Expands display tabs, decorates the cursor and computes final padding.
    fn row(&self, line: &LayoutLine, width: usize, padding: usize) -> String {
        if width == 0 {
            return String::new();
        }
        let owned = self.state.borrow().pastes.len();
        let mut text = crate::text::expand_tabs(&line.text);
        let budget = width - padding;
        if let Some(cursor) = line.cursor {
            let cursor = display_cursor(&line.text, cursor);
            let mapped = crate::text::expand_tabs(&line.text[..cursor]).len();
            text = Cow::Owned(decorate(&text, mapped, budget, self.focus.get(), owned));
        } else if visible_width(&text) > budget {
            text = Cow::Owned(crate::slice_by_column(&text, 0, budget, true));
        }
        let cells = visible_width(&text);
        format!(
            "{}{text}{}",
            " ".repeat(padding),
            " ".repeat(width.saturating_sub(padding + cells))
        )
    }
}
/// Retains the source floating-point order through flooring to a native line count.
pub(super) fn visible_lines(rows: usize) -> usize {
    super::normalize(
        rows.to_string().parse::<f64>().unwrap_or(0.0) * 0.3,
        5,
        usize::MAX,
        5,
    )
}
/// Decorates a whole grapheme, owned marker or blank, clipping only the displayed viewport.
fn decorate(text: &str, cursor: usize, budget: usize, focused: bool, owned: usize) -> String {
    let cursor = display_cursor(text, cursor);
    let at = markers::segments(&text[cursor..], owned)
        .first()
        .map_or(" ", |&(_, text)| text);
    let end = (cursor + at.len()).min(text.len());
    let marker = if focused { CURSOR_MARKER } else { "" };
    let result = format!(
        "{}{marker}\x1b[7m{at}\x1b[0m{}",
        &text[..cursor],
        &text[end..]
    );
    if visible_width(&result) <= budget {
        return result;
    }
    let before = crate::slice_by_column(&text[..cursor], 0, budget.saturating_sub(1), true);
    format!("{before}{marker}\x1b[7m \x1b[0m")
}
