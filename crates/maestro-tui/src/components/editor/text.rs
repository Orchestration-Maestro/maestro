//! Buffer normalization and cursor-local edits.
use super::{Buffer, Editing, Editor, LastAction};
use crate::{autocomplete::CursorPosition, get_segmenter, is_whitespace_char};
impl Editor {
    /// Replaces normalized text, captures changed content and always notifies.
    pub fn set_text(&self, text: &str) {
        let text = normalize(text);
        {
            let mut state = self.state.borrow_mut();
            state.reset_action();
            state.history_index = None;
            if state.current.lines.join("\n") != text {
                state.snapshot();
            }
            let lines = text.split('\n').map(str::to_owned).collect::<Vec<_>>();
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
    /// Splices normalized text as one undoable edit; empty input has no effect.
    pub fn insert_text_at_cursor(&self, text: &str) {
        if text.is_empty() {
            return;
        }
        {
            let mut state = self.state.borrow_mut();
            state.snapshot();
            state.reset_action();
            state.history_index = None;
            state.splice(&normalize(text));
        }
        self.notify();
    }
    /// Inserts a raw typed chunk with word coalescence.
    pub(super) fn type_text(&self, text: &str) {
        {
            let mut state = self.state.borrow_mut();
            if is_whitespace_char(text) || state.action != LastAction::TypeWord {
                state.snapshot();
            }
            state.action = LastAction::TypeWord;
            state.history_index = None;
            state.revision = state.revision.wrapping_add(1);
            let cursor = state.current.cursor;
            state.current.lines[cursor.line].insert_str(cursor.col, text);
            let col = state.current.cursor.col + text.len();
            state.set_col(col);
        }
        self.notify();
    }
}
impl Editing {
    /// Restores a snapshot, leaving empty undo silent.
    pub(super) fn undo(&mut self) -> bool {
        self.history_index = None;
        let Some(snapshot) = self.undo.pop() else {
            return false;
        };
        self.current = snapshot;
        self.set_col(self.current.cursor.col);
        self.reset_action();
        true
    }
    /// Captures owned text and cursor through the shared undo stack.
    pub(super) fn snapshot(&mut self) {
        self.undo.push(&self.current);
    }
    /// Inserts logical lines while preserving the current suffix.
    pub(super) fn splice(&mut self, text: &str) {
        self.current.splice(text);
        self.set_col(self.current.cursor.col);
    }
    /// Deletes a cursor-local grapheme or joins the preceding logical line.
    pub(super) fn backspace(&mut self) {
        self.history_index = None;
        self.reset_action();
        let cursor = self.current.cursor;
        if cursor.col > 0 {
            self.snapshot();
            let line = &mut self.current.lines[cursor.line];
            let start = get_segmenter(&line[..cursor.col])
                .last()
                .map_or(0, |(at, _)| at);
            line.replace_range(start..cursor.col, "");
            self.set_col(start);
        } else if cursor.line > 0 {
            self.snapshot();
            let removed = self.current.lines.remove(cursor.line);
            let previous = &mut self.current.lines[cursor.line - 1];
            self.current.cursor.col = previous.len();
            previous.push_str(&removed);
            self.current.cursor.line -= 1;
            self.set_col(self.current.cursor.col);
        }
    }
    /// Deletes the following grapheme or joins the following logical line.
    pub(super) fn delete(&mut self) {
        self.history_index = None;
        self.reset_action();
        let cursor = self.current.cursor;
        let line = &self.current.lines[cursor.line];
        if cursor.col < line.len() {
            let length = get_segmenter(&line[cursor.col..])
                .next()
                .map_or(0, |(_, text)| text.len());
            self.snapshot();
            self.current.lines[cursor.line].replace_range(cursor.col..cursor.col + length, "");
        } else if cursor.line + 1 < self.current.lines.len() {
            self.snapshot();
            let removed = self.current.lines.remove(cursor.line + 1);
            self.current.lines[cursor.line].push_str(&removed);
        }
    }
    /// Moves one grapheme right, crossing logical lines.
    pub(super) fn right(&mut self, width: usize) {
        self.reset_action();
        let previous = self.current.cursor;
        let cursor = &mut self.current.cursor;
        let line = &self.current.lines[cursor.line];
        if cursor.col < line.len() {
            cursor.col += get_segmenter(&line[cursor.col..])
                .next()
                .map_or(0, |(_, text)| text.len());
        } else if cursor.line + 1 < self.current.lines.len() {
            cursor.line += 1;
            cursor.col = 0;
        }
        if self.current.cursor == previous {
            self.remember_visual_column(width);
        } else {
            self.set_col(self.current.cursor.col);
        }
    }
    /// Moves one grapheme left, crossing logical lines.
    pub(super) fn left(&mut self) {
        self.reset_action();
        let previous = self.current.cursor;
        let cursor = &mut self.current.cursor;
        if cursor.col > 0 {
            cursor.col = get_segmenter(&self.current.lines[cursor.line][..cursor.col])
                .last()
                .map_or(0, |(at, _)| at);
        } else if cursor.line > 0 {
            cursor.line -= 1;
            cursor.col = self.current.lines[cursor.line].len();
        }
        if self.current.cursor != previous {
            self.set_col(self.current.cursor.col);
        }
    }
}
/// Normalizes programmatic storage, not ordinary input.
fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\t', "    ")
}

impl Buffer {
    /// Splices raw logical lines while retaining the current suffix.
    pub(super) fn splice(&mut self, text: &str) {
        let cursor = self.cursor;
        let suffix = self.lines[cursor.line].split_off(cursor.col);
        let mut inserted = text.split('\n');
        self.lines[cursor.line].push_str(inserted.next().unwrap_or(""));
        self.cursor.col = self.lines[cursor.line].len();
        for line in inserted {
            self.cursor.line += 1;
            self.lines.insert(self.cursor.line, line.to_owned());
            self.cursor.col = line.len();
        }
        self.lines[self.cursor.line].push_str(&suffix);
    }
}
