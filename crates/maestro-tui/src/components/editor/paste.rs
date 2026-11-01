//! Bracketed paste framing, cleaning and large-paste storage.
use super::{Editing, Owner, markers, text};
use crate::tui::InputHandler;

/// Begins a bracketed paste.
const START: &str = "\x1b[200~";
/// Ends a bracketed paste.
const END: &str = "\x1b[201~";
/// Largest paste kept inline, in logical lines.
const MAX_LINES: usize = 10;
/// Largest paste kept inline, in Unicode scalars.
const MAX_SCALARS: usize = 1000;

impl Owner {
    /// Substitutes each stored paste for its canonical markers in the current text.
    pub(super) fn get_expanded_text(&self) -> String {
        self.state.borrow().expand(&self.get_text())
    }

    /// Buffers input between paste markers and reports whether it was consumed.
    ///
    /// A start marker discards any unfinished paste; input after the first end
    /// marker is dispatched as ordinary input.
    pub(super) fn frame_paste(&self, data: &str) -> bool {
        let (payload, rest) = {
            let mut state = self.state.borrow_mut();
            if data.contains(START) {
                state.framing = Some(String::new());
            }
            let Some(buffer) = state.framing.as_mut() else {
                return false;
            };
            buffer.push_str(&data.replacen(START, "", 1));
            let Some(end) = buffer.find(END) else {
                return true;
            };
            let rest = buffer.split_off(end + END.len());
            buffer.truncate(end);
            (state.framing.take().unwrap_or_default(), rest)
        };
        if !payload.is_empty() {
            self.paste(&payload);
        }
        if !rest.is_empty() {
            self.handle_input(&rest);
        }
        true
    }

    /// Inserts one nonempty raw paste as a single undoable edit, even when cleaning leaves no text.
    fn paste(&self, raw: &str) {
        self.cancel_autocomplete();
        {
            let mut state = self.state.borrow_mut();
            state.history_index = None;
            state.reset_action();
            state.snapshot();
            let cursor = state.current.cursor;
            let before = state.current.lines[cursor.line][..cursor.col]
                .chars()
                .next_back();
            let cleaned = clean(raw, before);
            if cleaned.is_empty() {
                return;
            }
            let inserted = state.admit(cleaned);
            state.splice(&inserted);
        }
        self.notify();
    }
}

impl Editing {
    /// Substitutes each stored paste for its canonical markers, in creation order.
    pub(super) fn expand(&self, text: &str) -> String {
        self.pastes
            .iter()
            .enumerate()
            .fold(text.to_owned(), |text, (index, content)| {
                markers::expand(&text, index + 1, content)
            })
    }

    /// Stores a large paste and returns its marker; smaller pastes return unchanged.
    fn admit(&mut self, text: String) -> String {
        let lines = text.split('\n').count();
        let scalars = text.chars().count();
        if lines <= MAX_LINES && scalars <= MAX_SCALARS {
            return text;
        }
        self.pastes.push(text);
        let id = self.pastes.len();
        if lines > MAX_LINES {
            format!("[paste #{id} +{lines} lines]")
        } else {
            format!("[paste #{id} {scalars} chars]")
        }
    }
}

/// Decodes control letters, normalizes lines and tabs, drops other controls and spaces paths.
fn clean(raw: &str, before: Option<char>) -> String {
    let mut text: String = text::normalize(&decode_control_letters(raw))
        .chars()
        .filter(|&scalar| scalar == '\n' || scalar >= ' ')
        .collect();
    let word = before.is_some_and(|scalar| scalar.is_ascii_alphanumeric() || scalar == '_');
    if word && text.starts_with(['/', '~', '.']) {
        text.insert(0, ' ');
    }
    text
}

/// Rewrites `ESC [ code ; 5 u` for ASCII letters as the control byte a terminal encoded.
fn decode_control_letters(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("\x1b[") {
        result.push_str(&rest[..at]);
        let tail = &rest[at + 2..];
        let digits = tail.bytes().take_while(u8::is_ascii_digit).count();
        let control = tail[..digits]
            .parse::<u8>()
            .ok()
            .zip(tail[digits..].strip_prefix(";5u"))
            .and_then(|(code, next)| Some((control_byte(code)?, next)));
        if let Some((control, next)) = control {
            result.push(char::from(control));
            rest = next;
        } else {
            result.push_str("\x1b[");
            rest = tail;
        }
    }
    result.push_str(rest);
    result
}

/// Control byte for an ASCII letter code, regardless of case.
fn control_byte(code: u8) -> Option<u8> {
    code.is_ascii_alphabetic().then_some(code & 0x1f)
}
