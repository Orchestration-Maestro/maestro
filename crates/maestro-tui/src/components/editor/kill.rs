//! Directional deleted text and callback-separated ring rotation.
use super::{Editing, LastAction, Owner};
use crate::kill_ring::KillRingOptions;
/// Cursor-local kill selection.
#[derive(Clone, Copy)]
pub(super) enum Kill {
    /// Preceding word run.
    WordBackward,
    /// Following word run.
    WordForward,
    /// Prefix of the logical line.
    LineStart,
    /// Suffix of the logical line.
    LineEnd,
}
impl Editing {
    /// Deletes an admitted range, retaining direction and no-op action state.
    pub(super) fn kill(&mut self, kind: Kill) {
        self.history_index = None;
        let cursor = self.current.cursor;
        let backward = matches!(kind, Kill::WordBackward | Kill::LineStart);
        let length = self.current.lines[cursor.line].len();
        let accumulate = self.action == LastAction::Kill;
        let deleted = if backward && cursor.col == 0 && cursor.line > 0 {
            self.snapshot();
            let removed = self.current.lines.remove(cursor.line);
            let col = self.current.lines[cursor.line - 1].len();
            self.current.lines[cursor.line - 1].push_str(&removed);
            self.current.cursor.line -= 1;
            self.set_col(col);
            "\n".to_owned()
        } else if !backward && cursor.col == length && cursor.line + 1 < self.current.lines.len() {
            self.snapshot();
            let removed = self.current.lines.remove(cursor.line + 1);
            self.current.lines[cursor.line].push_str(&removed);
            "\n".to_owned()
        } else if (backward && cursor.col > 0) || (!backward && cursor.col < length) {
            self.snapshot();
            let target = match kind {
                Kill::LineStart => 0,
                Kill::LineEnd => length,
                Kill::WordBackward | Kill::WordForward => {
                    self.word(!backward);
                    let target = self.current.cursor.col;
                    self.set_col(cursor.col);
                    target
                }
            };
            let range = cursor.col.min(target)..cursor.col.max(target);
            let deleted = self.current.lines[cursor.line].drain(range).collect();
            if backward {
                self.set_col(target);
            }
            deleted
        } else {
            return;
        };
        self.revision = self.revision.wrapping_add(1);
        self.ring.push(
            &deleted,
            KillRingOptions {
                prepend: backward,
                accumulate,
            },
        );
        self.action = LastAction::Kill;
    }
    /// Removes the latest yanked range while preserving both surrounding fragments.
    fn delete_yank(&mut self) {
        let Some(text) = self.ring.peek() else {
            return;
        };
        let lines = text.split('\n').collect::<Vec<_>>();
        let cursor = self.current.cursor;
        let line = cursor.line - (lines.len() - 1);
        let col = if lines.len() == 1 {
            cursor.col - text.len()
        } else {
            self.current.lines[line].len() - lines[0].len()
        };
        let suffix = self.current.lines[cursor.line].split_off(cursor.col);
        self.current.lines[line].truncate(col);
        self.current.lines[line].push_str(&suffix);
        self.current.lines.drain(line + 1..=cursor.line);
        self.current.cursor.line = line;
        self.set_col(col);
        self.revision = self.revision.wrapping_add(1);
    }
}
impl Owner {
    /// Preserves deletion notification before reading and rotating the live ring.
    pub(super) fn yank(&self, rotate: bool) {
        {
            let mut state = self.state.borrow_mut();
            if state.ring.length() == 0
                || (rotate && (state.action != LastAction::Yank || state.ring.length() <= 1))
            {
                return;
            }
            state.snapshot();
            if rotate {
                state.delete_yank();
                state.action = LastAction::None;
            }
        }
        if rotate {
            self.notify();
        }
        let revision = {
            let mut state = self.state.borrow_mut();
            if rotate {
                state.ring.rotate();
            }
            state.history_index = None;
            let Editing { current, ring, .. } = &mut *state;
            current.splice(ring.peek().unwrap_or(""));
            let col = state.current.cursor.col;
            state.set_col(col);
            state.revision = state.revision.wrapping_add(1);
            state.revision
        };
        self.notify();
        let mut state = self.state.borrow_mut();
        if state.revision == revision {
            state.action = LastAction::Yank;
        }
    }
}
