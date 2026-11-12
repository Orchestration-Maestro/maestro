//! Retained prompt admission and browsing.
use super::{Buffer, Editing, Owner};
use crate::autocomplete::CursorPosition;
impl Owner {
    /// Adds a trimmed nonempty prompt, suppressing the newest duplicate and retaining 100 entries.
    pub(super) fn add_to_history(&self, text: &str) {
        let text = text.trim_matches(crate::text::utils::is_whitespace_scalar);
        let mut state = self.state.borrow_mut();
        if text.is_empty() || state.history.first().is_some_and(|latest| latest == text) {
            return;
        }
        state.history.insert(0, text.to_owned());
        state.history.truncate(100);
    }
    /// Commits an admitted history selection before notifying its observers.
    pub(super) fn browse_history(&self, forward: bool) {
        {
            let mut state = self.state.borrow_mut();
            state.reset_action();
            if state.history.is_empty() {
                return;
            }
            let index = match (state.history_index, forward) {
                (None, true) => return,
                (None, false) => Some(0),
                (Some(0), true) => None,
                (Some(index), true) => Some(index - 1),
                (Some(index), false) if index + 1 >= state.history.len() => return,
                (Some(index), false) => Some(index + 1),
            };
            if state.history_index.is_none() && index.is_some() {
                state.snapshot();
                state.draft = Some(state.current.clone());
            }
            state.history_index = index;
            match index {
                Some(index) => state.show_entry(index, forward),
                None => state.restore_draft(),
            }
        }
        self.scroll.set(0);
        self.notify();
    }
}
impl Editing {
    /// Shows history entry `index` with the cursor at its end when browsing forward and at its
    /// start when browsing backward.
    fn show_entry(&mut self, index: usize, forward: bool) {
        let lines: Vec<String> = self.history[index].split('\n').map(str::to_owned).collect();
        let line = if forward { lines.len() - 1 } else { 0 };
        let col = if forward { lines[line].len() } else { 0 };
        self.current = Buffer {
            lines,
            cursor: CursorPosition { line, col },
        };
        self.set_col(col);
    }
    /// Puts back the prompt and cursor that browsing began with, or an empty prompt.
    fn restore_draft(&mut self) {
        self.current = self.draft.take().unwrap_or_default();
        self.set_col(self.current.cursor.col);
    }
    /// Leaves history browsing and forgets the saved draft.
    pub(super) fn exit_history(&mut self) {
        self.history_index = None;
        self.draft = None;
    }
}
