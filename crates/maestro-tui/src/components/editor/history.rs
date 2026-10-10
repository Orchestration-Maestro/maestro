//! Retained prompt admission and browsing.
use super::{Buffer, Editor};
use crate::autocomplete::CursorPosition;
impl Editor {
    /// Adds a trimmed nonempty prompt, suppressing the newest duplicate and retaining 100 entries.
    pub fn add_to_history(&self, text: &str) {
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
            }
            state.history_index = index;
            let text = index.map_or("", |index| state.history[index].as_str());
            let lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
            let line = lines.len() - 1;
            let col = lines[line].len();
            state.current = Buffer {
                lines,
                cursor: CursorPosition { line, col },
            };
            state.set_col(col);
        }
        self.scroll.set(0);
        self.notify();
    }
}
