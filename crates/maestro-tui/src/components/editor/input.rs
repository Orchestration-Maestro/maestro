//! Registered key dispatch for basic editing.
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
    /// Delete following cluster.
    Delete,
    /// Current logical-line start.
    Home,
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
}
impl InputHandler for Editor {
    fn handle_input(&self, data: &str) {
        let bindings = get_keybindings();
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
                state.typing = false;
                state.current.cursor.col = if matches!(action, Action::Home) {
                    0
                } else {
                    state.current.lines[state.current.cursor.line].len()
                };
            }
            Action::Left => self.state.borrow_mut().left(),
            Action::Right => self.state.borrow_mut().right(),
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
    /// Inserts one newline after capturing a snapshot.
    fn newline(&self) {
        {
            let mut state = self.state.borrow_mut();
            state.snapshot();
            state.typing = false;
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
            state.undo.clear();
            state.typing = false;
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
/// Resolves current bindings before enhanced printable decoding.
fn action(data: &str, bindings: &KeybindingsManager) -> Option<Action> {
    let first = [
        ("tui.input.copy", Action::Consume),
        ("tui.editor.undo", Action::Undo),
        ("tui.input.tab", Action::Consume),
        ("tui.editor.deleteCharBackward", Action::Backspace),
        ("tui.editor.deleteCharForward", Action::Delete),
        ("tui.editor.cursorLineStart", Action::Home),
        ("tui.editor.cursorLineEnd", Action::End),
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
        ("tui.editor.cursorRight", Action::Right),
        ("tui.editor.cursorLeft", Action::Left),
    ] {
        if bindings.matches(data, binding) {
            return Some(action);
        }
    }
    matches_key(data, "shift+space").then_some(Action::Space)
}
