//! Buffer normalization and cursor-local edits.
use super::{Buffer, Editing, Editor};
use crate::{autocomplete::CursorPosition, get_segmenter, is_whitespace_char};
impl Editor {
    /// Replaces normalized text, captures changed content and always notifies.
    pub fn set_text(&self, text: &str) {
        let text = normalize(text);
        {
            let mut state = self.state.borrow_mut();
            state.typing = false;
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
            state.typing = false;
            state.splice(&normalize(text));
        }
        self.notify();
    }
    /// Inserts a raw typed chunk with word coalescence.
    pub(super) fn type_text(&self, text: &str) {
        {
            let mut state = self.state.borrow_mut();
            if is_whitespace_char(text) || !state.typing {
                state.snapshot();
            }
            state.typing = true;
            let cursor = state.current.cursor;
            state.current.lines[cursor.line].insert_str(cursor.col, text);
            state.current.cursor.col += text.len();
        }
        self.notify();
    }
}
impl Editing {
    /// Restores a snapshot, leaving empty undo silent.
    pub(super) fn undo(&mut self) -> bool {
        let Some(snapshot) = self.undo.pop() else {
            return false;
        };
        self.current = snapshot;
        self.typing = false;
        true
    }
    /// Captures owned text and cursor through the shared undo stack.
    pub(super) fn snapshot(&mut self) {
        self.undo.push(&self.current);
    }
    /// Inserts logical lines while preserving the current suffix.
    pub(super) fn splice(&mut self, text: &str) {
        let cursor = self.current.cursor;
        let suffix = self.current.lines[cursor.line].split_off(cursor.col);
        let mut inserted = text.split('\n');
        self.current.lines[cursor.line].push_str(inserted.next().unwrap_or(""));
        self.current.cursor.col = self.current.lines[cursor.line].len();
        for line in inserted {
            self.current.cursor.line += 1;
            self.current
                .lines
                .insert(self.current.cursor.line, line.to_owned());
            self.current.cursor.col = line.len();
        }
        self.current.lines[self.current.cursor.line].push_str(&suffix);
    }
    /// Deletes a cursor-local grapheme or joins the preceding logical line.
    pub(super) fn backspace(&mut self) {
        self.typing = false;
        let cursor = self.current.cursor;
        if cursor.col > 0 {
            self.snapshot();
            let line = &mut self.current.lines[cursor.line];
            let start = get_segmenter(&line[..cursor.col])
                .last()
                .map_or(0, |(at, _)| at);
            line.replace_range(start..cursor.col, "");
            self.current.cursor.col = start;
        } else if cursor.line > 0 {
            self.snapshot();
            let removed = self.current.lines.remove(cursor.line);
            let previous = &mut self.current.lines[cursor.line - 1];
            self.current.cursor.col = previous.len();
            previous.push_str(&removed);
            self.current.cursor.line -= 1;
        }
    }
    /// Deletes the following grapheme or joins the following logical line.
    pub(super) fn delete(&mut self) {
        self.typing = false;
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
    pub(super) fn right(&mut self) {
        self.typing = false;
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
    }
    /// Moves one grapheme left, crossing logical lines.
    pub(super) fn left(&mut self) {
        self.typing = false;
        let cursor = &mut self.current.cursor;
        if cursor.col > 0 {
            cursor.col = get_segmenter(&self.current.lines[cursor.line][..cursor.col])
                .last()
                .map_or(0, |(at, _)| at);
        } else if cursor.line > 0 {
            cursor.line -= 1;
            cursor.col = self.current.lines[cursor.line].len();
        }
    }
}
/// Normalizes programmatic storage, not ordinary input.
fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\t', "    ")
}
