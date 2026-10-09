//! Private text transitions using cursor-local graphemes.

use super::{Editing, LastAction, PasteChunk};
use crate::{
    KeybindingsManager, get_segmenter, is_punctuation_char, is_whitespace_char,
    kill_ring::KillRingOptions,
};

impl Editing {
    /// Dispatches character, word and line deletions in precedence order.
    pub(super) fn deletion(&mut self, data: &str, bindings: &KeybindingsManager) -> bool {
        if bindings.matches(data, "tui.editor.deleteCharBackward") {
            self.backspace();
        } else if bindings.matches(data, "tui.editor.deleteCharForward") {
            self.forward_delete();
        } else if bindings.matches(data, "tui.editor.deleteWordBackward") {
            self.kill_word_backward();
        } else if bindings.matches(data, "tui.editor.deleteWordForward") {
            self.kill_word_forward();
        } else if bindings.matches(data, "tui.editor.deleteToLineStart") {
            self.kill(0..self.current.cursor, true);
        } else if bindings.matches(data, "tui.editor.deleteToLineEnd") {
            self.kill(self.current.cursor..self.current.value.len(), false);
        } else {
            return false;
        }
        true
    }

    /// Dispatches yank before rotation.
    pub(super) fn yanking(&mut self, data: &str, bindings: &KeybindingsManager) -> bool {
        if bindings.matches(data, "tui.editor.yank") {
            self.yank();
        } else if bindings.matches(data, "tui.editor.yankPop") {
            self.yank_pop();
        } else {
            return false;
        }
        true
    }

    /// Dispatches character, line and word movements in precedence order.
    pub(super) fn movement(&mut self, data: &str, bindings: &KeybindingsManager) -> bool {
        if bindings.matches(data, "tui.editor.cursorLeft") {
            self.left();
        } else if bindings.matches(data, "tui.editor.cursorRight") {
            self.right();
        } else if bindings.matches(data, "tui.editor.cursorLineStart") {
            self.last = LastAction::None;
            self.current.cursor = 0;
        } else if bindings.matches(data, "tui.editor.cursorLineEnd") {
            self.last = LastAction::None;
            self.current.cursor = self.current.value.len();
        } else if bindings.matches(data, "tui.editor.cursorWordLeft") {
            self.move_word_left();
        } else if bindings.matches(data, "tui.editor.cursorWordRight") {
            self.move_word_right();
        } else {
            return false;
        }
        true
    }

    /// Buffers paste markers and inserts completed payload atomically.
    pub(super) fn paste_chunk(&mut self, data: &str) -> PasteChunk {
        let data = if data.contains("\x1b[200~") {
            self.paste = Some(String::new());
            std::borrow::Cow::Owned(data.replacen("\x1b[200~", "", 1))
        } else {
            std::borrow::Cow::Borrowed(data)
        };
        let Some(buffer) = self.paste.as_mut() else {
            return PasteChunk::Ordinary;
        };
        buffer.push_str(&data);
        let Some(end) = buffer.find("\x1b[201~") else {
            return PasteChunk::Pending;
        };
        let suffix = buffer[end + 6..].to_owned();
        let clean = buffer[..end]
            .replace(['\r', '\n'], "")
            .replace('\t', "    ");
        self.paste = None;
        self.last = LastAction::None;
        self.undo.push(&self.current);
        self.current.value.insert_str(self.current.cursor, &clean);
        self.current.cursor += clean.len();
        PasteChunk::Suffix(suffix)
    }

    /// Restores a captured value and cursor without rewinding deleted-text history.
    pub(super) fn undo(&mut self) {
        if let Some(snapshot) = self.undo.pop() {
            self.current = snapshot;
            self.last = LastAction::None;
        }
    }

    /// Deletes the final grapheme of the current prefix.
    fn backspace(&mut self) {
        self.last = LastAction::None;
        if let Some((start, _)) = get_segmenter(&self.current.value[..self.current.cursor]).last() {
            self.undo.push(&self.current);
            self.current
                .value
                .replace_range(start..self.current.cursor, "");
            self.current.cursor = start;
        }
    }

    /// Deletes the first grapheme of the current suffix.
    fn forward_delete(&mut self) {
        self.last = LastAction::None;
        let length = get_segmenter(&self.current.value[self.current.cursor..])
            .next()
            .map(|(_, text)| text.len());
        if let Some(length) = length {
            let end = self.current.cursor + length;
            self.undo.push(&self.current);
            self.current
                .value
                .replace_range(self.current.cursor..end, "");
        }
    }

    /// Inserts at the current byte cursor.
    pub(super) fn insert(&mut self, text: &str) {
        if is_whitespace_char(text) || self.last != LastAction::TypeWord {
            self.undo.push(&self.current);
        }
        self.last = LastAction::TypeWord;
        self.current.value.insert_str(self.current.cursor, text);
        self.current.cursor += text.len();
    }

    /// Finds the start of the preceding word or punctuation run.
    fn word_left(&self) -> usize {
        let segments: Vec<_> = get_segmenter(&self.current.value[..self.current.cursor])
            .map(|(_, text)| text)
            .collect();
        self.current.cursor - word_distance(segments.into_iter().rev())
    }

    /// Finds the end of the following word or punctuation run.
    fn word_right(&self) -> usize {
        self.current.cursor
            + word_distance(
                get_segmenter(&self.current.value[self.current.cursor..]).map(|(_, text)| text),
            )
    }

    /// Saves and removes the following word.
    fn kill_word_forward(&mut self) {
        let end = self.word_right();
        self.kill(self.current.cursor..end, false);
    }

    /// Moves one grapheme through the current prefix, resetting at its boundary too.
    fn left(&mut self) {
        self.last = LastAction::None;
        if let Some((start, _)) = get_segmenter(&self.current.value[..self.current.cursor]).last() {
            self.current.cursor = start;
        }
    }

    /// Moves over one preceding word, leaving coalescence intact at the start.
    fn move_word_left(&mut self) {
        if self.current.cursor == 0 {
            return;
        }
        self.last = LastAction::None;
        self.current.cursor = self.word_left();
    }

    /// Moves over one following word, leaving coalescence intact at the end.
    fn move_word_right(&mut self) {
        if self.current.cursor == self.current.value.len() {
            return;
        }
        self.last = LastAction::None;
        self.current.cursor = self.word_right();
    }

    /// Moves one grapheme through the current suffix.
    fn right(&mut self) {
        self.last = LastAction::None;
        self.current.cursor += get_segmenter(&self.current.value[self.current.cursor..])
            .next()
            .map_or(0, |(_, text)| text.len());
    }

    /// Saves and removes a nonempty range.
    fn kill(&mut self, range: std::ops::Range<usize>, prepend: bool) {
        if range.is_empty() {
            return;
        }
        self.undo.push(&self.current);
        self.ring.push(
            &self.current.value[range.clone()],
            KillRingOptions {
                prepend,
                accumulate: self.last == LastAction::Kill,
            },
        );
        self.last = LastAction::Kill;
        self.current.cursor = range.start;
        self.current.value.replace_range(range, "");
    }

    /// Saves and removes the preceding word.
    fn kill_word_backward(&mut self) {
        let start = self.word_left();
        self.kill(start..self.current.cursor, true);
    }

    /// Replaces the active yank with the next ring entry.
    fn yank_pop(&mut self) {
        if self.last != LastAction::Yank || self.ring.length() <= 1 {
            return;
        }
        self.undo.push(&self.current);
        let length = self.ring.peek().map_or(0, str::len);
        let start = self.current.cursor - length;
        self.current
            .value
            .replace_range(start..self.current.cursor, "");
        self.current.cursor = start;
        self.ring.rotate();
        self.insert_yank();
    }

    /// Inserts the newest deleted string.
    fn yank(&mut self) {
        if self.ring.peek().is_none() {
            return;
        }
        self.undo.push(&self.current);
        self.insert_yank();
    }

    /// Inserts the selected ring text without capturing another snapshot.
    fn insert_yank(&mut self) {
        if let Some(text) = self.ring.peek() {
            self.current.value.insert_str(self.current.cursor, text);
            self.current.cursor += text.len();
            self.last = LastAction::Yank;
        }
    }
}

/// Bytes in initial whitespace followed by one punctuation or word run.
fn word_distance<'a>(segments: impl Iterator<Item = &'a str>) -> usize {
    let mut segments = segments.peekable();
    let mut length = 0;
    while segments.peek().is_some_and(|text| is_whitespace_char(text)) {
        length += segments.next().map_or(0, str::len);
    }
    let punctuation = segments
        .peek()
        .is_some_and(|text| is_punctuation_char(text));
    while segments.peek().is_some_and(|text| {
        if punctuation {
            is_punctuation_char(text)
        } else {
            !is_whitespace_char(text) && !is_punctuation_char(text)
        }
    }) {
        length += segments.next().map_or(0, str::len);
    }
    length
}
