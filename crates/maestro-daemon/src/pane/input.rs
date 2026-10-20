//! Field projection from daemon input records into engine events.
use crate::input::TerminalKey;
use maestro_protocol::frames::{ClientKeyCode, ClientKeyKind};
use maestro_terminal::input::key::{Key, KeyAction, KeyEvent, Mods};

/// Project a semantic key without guessing a physical keyboard layout.
pub(super) fn key_event(key: &TerminalKey) -> KeyEvent {
    let mut event = KeyEvent {
        key: match key.code {
            ClientKeyCode::Backspace => Key::Backspace,
            ClientKeyCode::Enter => Key::Enter,
            ClientKeyCode::Left => Key::ArrowLeft,
            ClientKeyCode::Right => Key::ArrowRight,
            ClientKeyCode::Up => Key::ArrowUp,
            ClientKeyCode::Down => Key::ArrowDown,
            ClientKeyCode::Home => Key::Home,
            ClientKeyCode::End => Key::End,
            ClientKeyCode::PageUp => Key::PageUp,
            ClientKeyCode::PageDown => Key::PageDown,
            ClientKeyCode::Tab | ClientKeyCode::BackTab => Key::Tab,
            ClientKeyCode::Delete => Key::Delete,
            ClientKeyCode::Insert => Key::Insert,
            ClientKeyCode::Esc => Key::Escape,
            ClientKeyCode::F(number) => Key::function(number).unwrap_or_default(),
            ClientKeyCode::Char(_) | ClientKeyCode::Null => Key::Unidentified,
        },
        action: match key.kind {
            ClientKeyKind::Press => KeyAction::Press,
            ClientKeyKind::Repeat => KeyAction::Repeat,
            ClientKeyKind::Release => KeyAction::Release,
        },
        mods: Mods::from_int(u16::from(key.modifiers & 0x3f)),
        ..KeyEvent::default()
    };
    if key.code == ClientKeyCode::BackTab {
        event.mods = event.mods.union(Mods::SHIFT);
    }
    if let ClientKeyCode::Char(ch) = key.code {
        event.unshifted_codepoint = u32::from(ch);
        event.utf8 = key
            .shifted_codepoint
            .and_then(char::from_u32)
            .unwrap_or(ch)
            .to_string();
    }
    event
}

/// Encode semantic fallback input using the single engine encoder.
pub(super) fn encode_terminal_key(
    key: &TerminalKey,
    protocol: crate::input::KeyboardProtocol,
) -> Vec<u8> {
    use maestro_terminal::{
        input::key_encode::{KeyEncoder, KeyEncoding, OptionAsAlt},
        kitty::key::Flags,
    };
    if key.kind != ClientKeyKind::Release
        && let Some(text) = &key.generated_text
    {
        return text.as_bytes().to_vec();
    }
    let flags = match protocol {
        crate::input::KeyboardProtocol::Legacy => 0,
        crate::input::KeyboardProtocol::Kitty { flags } => flags,
    };
    let mut encoder = KeyEncoder {
        encoding: KeyEncoding::Semantic,
        kitty_flags: Flags::from_int(flags & 31).unwrap_or_default(),
        macos_option_as_alt: OptionAsAlt::True,
        ..KeyEncoder::default()
    };
    encoder
        .modes
        .set(maestro_terminal::Mode::AltEscPrefix, true);
    encoder.encode(&key_event(key))
}
