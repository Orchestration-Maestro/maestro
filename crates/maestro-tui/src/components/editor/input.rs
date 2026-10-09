//! Registered key dispatch for basic editing.
use super::kill::Kill;
use super::navigation::Direction;
use super::{Buffer, Editor};
use crate::{KeybindingsManager, get_keybindings, matches_key, tui::InputHandler};
/// Basic actions in dispatch precedence order.
#[derive(Clone, Copy)]
enum Action {
    /// Parent copy or provider-less Tab.
    Consume,
    /// Restore a snapshot.
    Undo,
    /// Delete preceding cluster.
    Backspace,
    /// Directional deletion retained in the ring.
    Kill(Kill),
    /// Insert the newest deleted text.
    Yank,
    /// Replace the preceding yank with the previous entry.
    YankPop,
    /// Delete following cluster.
    Delete,
    /// Current logical-line start.
    Home,
    /// Previous word run.
    WordLeft,
    /// Next word run.
    WordRight,
    /// Current logical-line end.
    End,
    /// Insert one logical newline.
    Newline,
    /// Submit unless gated.
    Submit,
    /// Next cluster.
    Right,
    /// Previous cluster.
    Left,
    /// Insert a literal space.
    Space,
    /// Previous prompt.
    Up,
    /// Next prompt.
    Down,
    /// Previous visual page.
    PageUp,
    /// Next visual page.
    PageDown,
    /// Wait for a literal search target.
    Jump(Direction),
}
impl InputHandler for Editor {
    fn handle_input(&self, data: &str) {
        let bindings = get_keybindings();
        if self.pending_jump(data, &bindings) {
            return;
        }
        if let Some(action) = action(data, &bindings) {
            self.apply(action, data, &bindings);
        } else if let Some(text) = crate::keys::decode_printable_key(data) {
            self.type_text(text.encode_utf8(&mut [0; 4]));
        } else if data.chars().next().is_some_and(|scalar| scalar >= ' ') {
            self.type_text(data);
        }
    }
}
impl Editor {
    /// Applies one action, releasing mutable storage before notification.
    fn apply(&self, action: Action, data: &str, bindings: &KeybindingsManager) {
        match action {
            Action::Consume => {}
            Action::Jump(direction) => self.state.borrow_mut().jump = Some(direction),
            Action::Up | Action::Down | Action::PageUp | Action::PageDown => self.navigate(action),
            Action::Kill(kind) => {
                self.state.borrow_mut().kill(kind);
                self.notify();
            }
            Action::Yank => self.yank(false),
            Action::YankPop => self.yank(true),
            Action::Undo => {
                let changed = self.state.borrow_mut().undo();
                if changed {
                    self.notify();
                }
            }
            Action::Backspace => {
                self.state.borrow_mut().backspace();
                self.notify();
            }
            Action::Delete => {
                self.state.borrow_mut().delete();
                self.notify();
            }
            Action::Home | Action::End => {
                let mut state = self.state.borrow_mut();
                state.reset_action();
                let col = if matches!(action, Action::Home) {
                    0
                } else {
                    state.current.lines[state.current.cursor.line].len()
                };
                state.set_col(col);
            }
            Action::WordLeft | Action::WordRight => self
                .state
                .borrow_mut()
                .word(matches!(action, Action::WordRight)),
            Action::Left => self.state.borrow_mut().left(),
            Action::Right => self.state.borrow_mut().right(self.width.get()),
            Action::Space => self.type_text(" "),
            Action::Newline => self.newline_input(data, bindings),
            Action::Submit => {
                if self.disabled.get() {
                    return;
                }
                if self.adjacent_backslash() {
                    self.erase_backslash();
                    self.newline();
                } else {
                    self.submit_value();
                }
            }
        }
    }
    /// Applies visual steps and pages using the retained width and live rows.
    fn navigate(&self, action: Action) {
        match action {
            Action::Up => self.vertical_input(false),
            Action::Down => self.vertical_input(true),
            Action::PageUp | Action::PageDown => {
                let count = super::render::visible_lines(self.terminal.borrow().rows());
                let direction = if matches!(action, Action::PageDown) {
                    Direction::Forward
                } else {
                    Direction::Backward
                };
                self.state
                    .borrow_mut()
                    .vertical(self.width.get(), direction, count, true);
            }
            _ => {}
        }
    }
    /// Inserts one newline after capturing a snapshot.
    fn newline(&self) {
        {
            let mut state = self.state.borrow_mut();
            state.snapshot();
            state.reset_action();
            state.history_index = None;
            state.splice("\n");
        }
        self.notify();
    }
    /// Handles rebound physical Enter with shifted submission.
    fn newline_input(&self, data: &str, bindings: &KeybindingsManager) {
        let shifted = bindings
            .get_keys("tui.input.submit")
            .iter()
            .any(|key| matches!(key.as_str(), "shift+enter" | "shift+return"));
        if !self.disabled.get()
            && matches_key(data, "enter")
            && shifted
            && self.adjacent_backslash()
        {
            self.erase_backslash();
            self.submit_value();
        } else {
            self.newline();
        }
    }
    /// Tests the stored character immediately preceding the cursor.
    fn adjacent_backslash(&self) -> bool {
        let state = self.state.borrow();
        let cursor = state.current.cursor;
        state.current.lines[cursor.line][..cursor.col].ends_with('\\')
    }
    /// Removes exactly one adjacent backslash and notifies before the next action.
    fn erase_backslash(&self) {
        self.state.borrow_mut().backspace();
        self.notify();
    }
    /// Clears state before change notification and selects the live submission callable afterward.
    fn submit_value(&self) {
        let value = self
            .get_text()
            .trim_matches(crate::text::utils::is_whitespace_scalar)
            .to_owned();
        {
            let mut state = self.state.borrow_mut();
            state.current = Buffer::default();
            state.history_index = None;
            state.undo.clear();
            state.reset_action();
        }
        self.scroll.set(0);
        if let Some(callback) = self.on_change() {
            callback("");
        }
        if let Some(callback) = self.on_submit() {
            callback(&value);
        }
    }
}
/// Resolves deletions and line/word actions before newline dispatch.
fn initial_action(data: &str, bindings: &KeybindingsManager) -> Option<Action> {
    let first = [
        ("tui.input.copy", Action::Consume),
        ("tui.editor.undo", Action::Undo),
        ("tui.input.tab", Action::Consume),
        ("tui.editor.deleteToLineEnd", Action::Kill(Kill::LineEnd)),
        (
            "tui.editor.deleteToLineStart",
            Action::Kill(Kill::LineStart),
        ),
        (
            "tui.editor.deleteWordBackward",
            Action::Kill(Kill::WordBackward),
        ),
        (
            "tui.editor.deleteWordForward",
            Action::Kill(Kill::WordForward),
        ),
        ("tui.editor.deleteCharBackward", Action::Backspace),
        ("tui.editor.deleteCharForward", Action::Delete),
        ("tui.editor.yank", Action::Yank),
        ("tui.editor.yankPop", Action::YankPop),
        ("tui.editor.cursorLineStart", Action::Home),
        ("tui.editor.cursorLineEnd", Action::End),
        ("tui.editor.cursorWordLeft", Action::WordLeft),
        ("tui.editor.cursorWordRight", Action::WordRight),
    ];
    for (binding, action) in first {
        let alias = match action {
            Action::Backspace => matches_key(data, "shift+backspace"),
            Action::Delete => matches_key(data, "shift+delete"),
            _ => false,
        };
        if bindings.matches(data, binding) || alias {
            return Some(action);
        }
    }
    None
}
/// Resolves later actions after the leading precedence group.
fn action(data: &str, bindings: &KeybindingsManager) -> Option<Action> {
    if let Some(action) = initial_action(data, bindings) {
        return Some(action);
    }
    if bindings.matches(data, "tui.input.newLine")
        || data.starts_with('\n')
        || data == "\x1b\r"
        || data == "\x1b[13;2~"
        || (data.contains('\x1b') && data.contains('\r'))
    {
        return Some(Action::Newline);
    }
    for (binding, action) in [
        ("tui.input.submit", Action::Submit),
        ("tui.editor.cursorUp", Action::Up),
        ("tui.editor.cursorDown", Action::Down),
        ("tui.editor.cursorRight", Action::Right),
        ("tui.editor.cursorLeft", Action::Left),
    ] {
        if bindings.matches(data, binding) {
            return Some(action);
        }
    }
    for (binding, action) in [
        ("tui.editor.pageUp", Action::PageUp),
        ("tui.editor.pageDown", Action::PageDown),
        ("tui.editor.jumpForward", Action::Jump(Direction::Forward)),
        ("tui.editor.jumpBackward", Action::Jump(Direction::Backward)),
    ] {
        if bindings.matches(data, binding) {
            return Some(action);
        }
    }
    matches_key(data, "shift+space").then_some(Action::Space)
}
