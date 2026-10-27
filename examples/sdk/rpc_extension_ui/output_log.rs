//! Bounded streamed text and cell-aware viewport clipping.
use maestro_tui::{Component, TruncateOptions, truncate_to_width};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};

/// A bounded log that clips whole terminal cells rather than text bytes.
#[derive(Default)]
pub(super) struct OutputLog {
    /// At most the authored thousand retained lines.
    lines: RefCell<VecDeque<String>>,
    /// Rows available above the prompt or dialog.
    pub(super) visible: Cell<usize>,
}
impl OutputLog {
    /// Appends one logical line and retires the oldest overflow.
    pub(super) fn append(&self, line: impl Into<String>) {
        let mut lines = self.lines.borrow_mut();
        lines.push_back(line.into());
        if lines.len() > 1000 {
            lines.pop_front();
        }
    }
    /// Appends untrusted text without interpreting its terminal controls or line breaks.
    pub(super) fn append_literal(&self, text: &str) {
        for line in text.split('\n') {
            self.append(Self::literal(line));
        }
    }
    /// Drops control introducers even when an escape is split across streamed chunks.
    pub(super) fn literal(text: &str) -> String {
        let text: String = text
            .chars()
            .filter(|scalar| !scalar.is_control() || *scalar == '\t')
            .collect();
        maestro_tui::text::expand_tabs(&text).into_owned()
    }
    /// Extends streamed text without inventing line breaks between chunks.
    pub(super) fn append_raw(&self, text: &str) {
        for (index, part) in text.split('\n').enumerate() {
            if index > 0 || self.lines.borrow().is_empty() {
                self.append("");
            }
            if let Some(line) = self.lines.borrow_mut().back_mut() {
                line.push_str(&Self::literal(part));
            }
        }
    }
}
impl Component for OutputLog {
    fn render(&self, width: usize) -> Vec<String> {
        let lines = self.lines.borrow();
        lines
            .iter()
            .skip(lines.len().saturating_sub(self.visible.get().max(1)))
            .map(|line| {
                truncate_to_width(
                    line,
                    width,
                    TruncateOptions {
                        ellipsis: "",
                        pad: false,
                    },
                )
            })
            .collect()
    }
}
