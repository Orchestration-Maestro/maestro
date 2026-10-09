//! Single-line editable text with shared editing history.

mod editing;
mod render;

use crate::{
    get_keybindings,
    kill_ring::KillRing,
    tui::{Component, FocusFlag, Focusable, InputHandler},
    undo_stack::UndoStack,
};
use std::{cell::RefCell, rc::Rc};

/// A synchronous submission callback.
type SubmitCallback = Rc<dyn Fn(&str)>;

/// Owned text and its byte cursor.
#[derive(Clone, Default)]
struct InputState {
    /// Current text.
    value: String,
    /// UTF-8 byte position in the text.
    cursor: usize,
}

/// Single-line terminal input.
#[derive(Default)]
pub struct Input {
    /// Current text and history.
    state: RefCell<Editing>,
    /// Current submission callback.
    submit: RefCell<Option<SubmitCallback>>,
    /// Keyboard focus set by the frame writer.
    focus: FocusFlag,
    /// Current cancellation callback.
    escape: RefCell<Option<Rc<dyn Fn()>>>,
}

/// Most recent coalescing transition.
#[derive(Default, PartialEq, Eq)]
enum LastAction {
    /// No active chain.
    #[default]
    None,
    /// Consecutive deleted-text operations.
    Kill,
    /// Typing coalescence after an inserted chunk.
    TypeWord,
    /// A yank or rotation.
    Yank,
}

/// Private editing storage.
#[derive(Default)]
struct Editing {
    /// Text and cursor captured together.
    current: InputState,
    /// Deleted strings available for yanking.
    ring: KillRing,
    /// Current coalescing action.
    last: LastAction,
    /// Owned text snapshots.
    undo: UndoStack<InputState>,
    /// Unfinished bracketed paste payload.
    paste: Option<String>,
}

impl Input {
    /// Creates empty input.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the current text.
    #[must_use]
    pub fn get_value(&self) -> String {
        self.state.borrow().current.value.clone()
    }

    /// Replaces text, retaining or clamping the logical cursor position.
    pub fn set_value(&self, value: String) {
        let mut state = self.state.borrow_mut();
        let logical = state.current.value[..state.current.cursor]
            .encode_utf16()
            .count();
        let mut consumed = 0;
        let cursor = value
            .char_indices()
            .find_map(|(byte, scalar)| {
                consumed += scalar.len_utf16();
                (consumed > logical).then_some(byte)
            })
            .unwrap_or(value.len());
        if state.current.value != value && state.last == LastAction::Yank {
            state.last = LastAction::None;
        }
        state.current = InputState { value, cursor };
    }

    /// Returns a retained submission callable.
    #[must_use]
    pub fn on_submit(&self) -> Option<SubmitCallback> {
        self.submit.borrow().clone()
    }

    /// Returns a retained cancellation callable.
    #[must_use]
    pub fn on_escape(&self) -> Option<Rc<dyn Fn()>> {
        self.escape.borrow().clone()
    }

    /// Replaces or removes the cancellation callback.
    pub fn set_on_escape(&self, callback: Option<Rc<dyn Fn()>>) {
        let old = self.escape.replace(callback);
        drop(old);
    }

    /// Replaces the submission callback.
    pub fn set_on_submit(&self, callback: Option<SubmitCallback>) {
        let old = self.submit.replace(callback);
        drop(old);
    }
}

impl InputHandler for Input {
    fn handle_input(&self, data: &str) {
        let mut data = std::borrow::Cow::Borrowed(data);
        loop {
            let paste = self.state.borrow_mut().paste_chunk(&data);
            match paste {
                PasteChunk::Ordinary => break,
                PasteChunk::Pending => return,
                PasteChunk::Suffix(suffix) if suffix.is_empty() => return,
                PasteChunk::Suffix(suffix) => data = std::borrow::Cow::Owned(suffix),
            }
        }
        self.dispatch(&data);
    }
}

impl Input {
    /// Dispatches against the manager active for this non-paste operation.
    fn dispatch(&self, data: &str) {
        let bindings = get_keybindings();
        if bindings.matches(data, "tui.select.cancel") {
            if let Some(callback) = self.on_escape() {
                callback();
            }
        } else if bindings.matches(data, "tui.editor.undo") {
            self.state.borrow_mut().undo();
        } else if bindings.matches(data, "tui.input.submit") || data == "\n" {
            if let Some(callback) = self.on_submit() {
                let value = self.get_value();
                callback(&value);
            }
        } else {
            let mut state = self.state.borrow_mut();
            if state.deletion(data, &bindings)
                || state.yanking(data, &bindings)
                || state.movement(data, &bindings)
            {
                return;
            }
            if let Some(scalar) = crate::decode_kitty_printable(data) {
                state.insert(scalar.encode_utf8(&mut [0; 4]));
            } else if !data
                .chars()
                .any(|scalar| matches!(scalar, '\u{0}'..='\u{1f}' | '\u{7f}'..='\u{9f}'))
            {
                state.insert(data);
            }
        }
    }
}

/// Result of consuming one bracketed-paste chunk.
enum PasteChunk {
    /// Ordinary input dispatch is needed.
    Ordinary,
    /// More paste payload is needed.
    Pending,
    /// Complete paste was inserted; dispatch this suffix afresh.
    Suffix(String),
}

impl Focusable for Input {
    fn focus_flag(&self) -> &FocusFlag {
        &self.focus
    }
}

impl Component for Input {
    fn render(&self, width: usize) -> Vec<String> {
        vec![render::line(
            &self.state.borrow().current,
            width,
            self.focus.get(),
        )]
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
    fn focusable(&self) -> Option<&dyn Focusable> {
        Some(self)
    }
}
