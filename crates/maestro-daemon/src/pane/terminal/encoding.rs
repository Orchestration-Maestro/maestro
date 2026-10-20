//! Destination repetition and live engine keyboard modes.
use super::PaneTerminal;
use crate::{
    input::{KeyboardProtocol, TerminalKey},
    pane::input::key_event,
};
use maestro_protocol::frames::ClientKeyKind;
use maestro_terminal::input::key_encode::KeyEncoder;

impl PaneTerminal {
    /// Encode grouped presses at the destination; a release is encoded once.
    #[must_use]
    pub fn encode_terminal_key(&self, key: TerminalKey, protocol: KeyboardProtocol) -> Vec<u8> {
        let Ok(core) = self.core.lock() else {
            return Vec::new();
        };
        let encoder = KeyEncoder::from_terminal(&core.terminal);
        let mut bytes = encode_once(&encoder, &key, protocol);
        if key.kind != ClientKeyKind::Release && key.repeat_count > 1 {
            let count = key.repeat_count;
            let repeated = key.with_kind(ClientKeyKind::Repeat);
            let repeat = encode_once(&encoder, &repeated, protocol);
            for _ in 1..count {
                bytes.extend_from_slice(&repeat);
            }
        }
        bytes
    }
}

/// Project one event onto the engine's keyboard encoder.
fn encode_once(encoder: &KeyEncoder, key: &TerminalKey, protocol: KeyboardProtocol) -> Vec<u8> {
    use maestro_protocol::frames::{ClientKeyCode, KeyModifiers};
    if matches!(key.code, ClientKeyCode::Char(_))
        || protocol == KeyboardProtocol::Legacy
            && key.code == ClientKeyCode::Tab
            && key.modifiers == KeyModifiers::CONTROL
    {
        return crate::pane::input::encode_terminal_key(key, protocol);
    }
    let mut event = key_event(key);
    event.mods = event.mods.binding();
    let bytes = encoder.encode(&event);
    let preserves_kind = !protocol.reports_event_types()
        || key.kind == ClientKeyKind::Press
        || std::str::from_utf8(&bytes)
            .ok()
            .and_then(crate::input::parse_terminal_key_sequence)
            .is_some_and(|parsed| {
                parsed.code == key.code
                    && parsed.modifiers == key.modifiers
                    && parsed.kind == key.kind
            });
    if bytes.is_empty() || !preserves_kind {
        crate::pane::input::encode_terminal_key(key, protocol)
    } else {
        bytes
    }
}
