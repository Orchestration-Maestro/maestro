//! Enhanced keyboard protocol sequences and committed text.

use std::fmt::Write as _;

use super::KeyEncoder;
use crate::input::key::{Key, KeyAction, KeyEvent};
use crate::input::key_mods::Mods;
use crate::input::kitty::{Entry, entry};
use crate::kitty::key::Flags;

/// Test whether a protocol enhancement is enabled.
fn enabled(opts: &KeyEncoder, flag: Flags) -> bool {
    opts.kitty_flags.int() & flag.int() != 0
}

/// Encode a key, preserving committed text and protocol opt-ins.
pub(super) fn encode(event: &KeyEvent, opts: &KeyEncoder) -> Vec<u8> {
    if event.action == KeyAction::Release
        && (!enabled(opts, Flags::REPORT_EVENTS)
            || (!enabled(opts, Flags::REPORT_ALL)
                && matches!(event.key, Key::Enter | Key::Backspace | Key::Tab)))
    {
        return Vec::new();
    }
    let entry = entry(event.key).or_else(|| {
        (event.unshifted_codepoint > 0).then_some(Entry {
            code: event.unshifted_codepoint,
            final_char: 'u',
            modifier: false,
        })
    });
    if event.composing {
        if !entry.is_some_and(|entry| entry.modifier) {
            return Vec::new();
        }
    } else if let Some(bytes) = plain_text(event, opts) {
        return bytes;
    }
    let Some(entry) = entry else {
        return event.utf8.as_bytes().to_vec();
    };
    if entry.modifier && !enabled(opts, Flags::REPORT_ALL) {
        return Vec::new();
    }
    Sequence::new(event, opts, entry).encode()
}

/// Handle composition confirmation and the protocol's legacy text exceptions.
fn plain_text(event: &KeyEvent, opts: &KeyEncoder) -> Option<Vec<u8>> {
    if !event.utf8.is_empty() && !super::legacy::control_text(&event.utf8) {
        match event.key {
            Key::Enter => return Some(event.utf8.as_bytes().to_vec()),
            Key::Backspace => return Some(Vec::new()),
            _ => {}
        }
    }
    if enabled(opts, Flags::REPORT_ALL) || !event.effective_mods().binding().empty() {
        return None;
    }
    match event.key {
        Key::Enter => return Some(vec![b'\r']),
        Key::Tab => return Some(vec![b'\t']),
        Key::Backspace => return Some(vec![0x7f]),
        _ => {}
    }
    if !event.utf8.is_empty()
        && event.action != KeyAction::Release
        && !event.utf8.chars().any(control)
    {
        return Some(event.utf8.as_bytes().to_vec());
    }
    None
}

/// Protocol controls include C0 and DEL, not all Unicode control categories.
fn control(ch: char) -> bool {
    ch.is_ascii_control()
}

/// A prepared wire sequence retaining borrowed associated text only during encoding.
struct Sequence<'a> {
    /// Functional or Unicode primary code and terminator.
    entry: Entry,
    /// Wire modifiers, already offset by one.
    mods: u16,
    /// Zero when event reporting is off, otherwise press/repeat/release.
    event: u8,
    /// Shifted and base-layout codepoints.
    alternates: [Option<u32>; 2],
    /// Associated text, empty when modifiers or releases prevent it.
    text: &'a str,
}

impl<'a> Sequence<'a> {
    /// Prepare the protocol fields from the caller's event.
    fn new(event: &'a KeyEvent, opts: &KeyEncoder, entry: Entry) -> Self {
        let mods = event.mods;
        let wire_mods = (mods.int() & 1)
            | ((mods.int() & 4) >> 1)
            | ((mods.int() & 2) << 1)
            | (mods.int() & 8)
            | ((mods.int() & 0x30) << 2);
        let action = if enabled(opts, Flags::REPORT_EVENTS) {
            match event.action {
                KeyAction::Press => 1,
                KeyAction::Repeat => 2,
                KeyAction::Release => 3,
            }
        } else {
            0
        };
        let prevents_text = mods.contains(Mods::CTRL)
            || mods.contains(Mods::SUPER)
            || (mods.contains(Mods::ALT) && opts.macos_option_as_alt.active(mods));
        Self {
            entry,
            mods: wire_mods + 1,
            event: action,
            alternates: if enabled(opts, Flags::REPORT_ALTERNATES) {
                alternates(event, entry.code)
            } else {
                [None; 2]
            },
            text: if enabled(opts, Flags::REPORT_ASSOCIATED) && action != 3 && !prevents_text {
                &event.utf8
            } else {
                ""
            },
        }
    }

    /// Render full Unicode/tilde sequences or short functional-key sequences.
    fn encode(&self) -> Vec<u8> {
        if !matches!(self.entry.final_char, 'u' | '~') {
            return self.special().into_bytes();
        }
        let mut result = format!("\x1b[{}", self.entry.code);
        if let Some(shifted) = self.alternates[0] {
            let _ = write!(result, ":{shifted}");
        }
        if let Some(base) = self.alternates[1] {
            if self.alternates[0].is_none() {
                result.push(':');
            }
            let _ = write!(result, ":{base}");
        }
        let prior = if self.event > 1 {
            let _ = write!(result, ";{}:{}", self.mods, self.event);
            true
        } else if self.mods > 1 {
            let _ = write!(result, ";{}", self.mods);
            true
        } else {
            false
        };
        self.associated(&mut result, prior);
        result.push(self.entry.final_char);
        result.into_bytes()
    }

    /// Append printable associated codepoints, retaining empty modifier fields.
    fn associated(&self, result: &mut String, prior: bool) {
        for (index, ch) in self.text.chars().filter(|&ch| !control(ch)).enumerate() {
            if index == 0 && !prior {
                result.push(';');
            }
            result.push(if index == 0 { ';' } else { ':' });
            result.push_str(&u32::from(ch).to_string());
        }
    }

    /// Functional keys always include the event type when requested, even presses.
    fn special(&self) -> String {
        let final_char = self.entry.final_char;
        if self.event != 0 {
            format!("\x1b[1;{}:{}{final_char}", self.mods, self.event)
        } else if self.mods > 1 {
            format!("\x1b[1;{}{final_char}", self.mods)
        } else {
            format!("\x1b[{final_char}")
        }
    }
}

/// Derive shifted and base-layout alternatives without reporting control codes.
fn alternates(event: &KeyEvent, code: u32) -> [Option<u32>; 2] {
    if code < 0x20 || code == 0x7f {
        return [None; 2];
    }
    let mut chars = event.utf8.chars();
    let first = chars.next().map(u32::from);
    let shifted = first.filter(|&ch| ch != code && event.mods.contains(Mods::SHIFT));
    let base = event.key.codepoint().map(u32::from).filter(|&base| {
        base != code && first.is_none_or(|first| first != base && chars.next().is_none())
    });
    [shifted, base]
}

#[cfg(test)]
mod tests;
