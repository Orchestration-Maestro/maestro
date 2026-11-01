//! Projection of structured toolkit keys onto the daemon key vocabulary.

use maestro_protocol::frames::{ClientKeyCode, ClientKeyKind, KeyModifiers};
use maestro_tui::keys::{Code, KeyEventType, Named, Sequence};

use super::TerminalKey;

/// Decode a complete key report without inventing transport evidence.
#[must_use]
pub fn parse_terminal_key_sequence(data: &str) -> Option<TerminalKey> {
    project_key(maestro_tui::keys::parse_terminal_key_sequence(data)?)
}

/// Retain supported identities, phase, shifted alternate and associated text.
fn project_key(sequence: Sequence) -> Option<TerminalKey> {
    let code = project_code(sequence.code)?;
    let kind = match sequence.event_type {
        KeyEventType::Press => ClientKeyKind::Press,
        KeyEventType::Repeat => ClientKeyKind::Repeat,
        KeyEventType::Release => ClientKeyKind::Release,
    };
    let mut key =
        TerminalKey::new(code, toolkit_modifiers_to_wire(sequence.modifier)).with_kind(kind);
    if let Some(shifted) = sequence.shifted {
        key = key.with_shifted_codepoint(shifted);
    }
    Some(key.with_generated_text(sequence.generated_text))
}

/// Map a toolkit key onto the daemon key vocabulary, absent for keys it has no code for.
pub(crate) fn project_code(code: Code) -> Option<ClientKeyCode> {
    Some(match code {
        Code::Char(ch) => ClientKeyCode::Char(ch),
        Code::Key(named) => match named {
            Named::Escape => ClientKeyCode::Esc,
            Named::Enter => ClientKeyCode::Enter,
            Named::Tab => ClientKeyCode::Tab,
            Named::BackTab => ClientKeyCode::BackTab,
            Named::Space => ClientKeyCode::Char(' '),
            Named::Backspace => ClientKeyCode::Backspace,
            Named::Insert => ClientKeyCode::Insert,
            Named::Delete => ClientKeyCode::Delete,
            Named::Left => ClientKeyCode::Left,
            Named::Right => ClientKeyCode::Right,
            Named::Up => ClientKeyCode::Up,
            Named::Down => ClientKeyCode::Down,
            Named::PageUp => ClientKeyCode::PageUp,
            Named::PageDown => ClientKeyCode::PageDown,
            Named::Home => ClientKeyCode::Home,
            Named::End => ClientKeyCode::End,
            Named::Function(number) => ClientKeyCode::F(number),
            _ => return None,
        },
    })
}

/// Convert the toolkit's Alt/Control layout, dropping lock bits.
pub(crate) fn toolkit_modifiers_to_wire(modifier: u32) -> u8 {
    let mut wire = 0;
    for (bit, mapped) in [
        (1, KeyModifiers::SHIFT),
        (2, KeyModifiers::ALT),
        (4, KeyModifiers::CONTROL),
        (8, KeyModifiers::SUPER),
        (16, KeyModifiers::HYPER),
        (32, KeyModifiers::META),
    ] {
        if modifier & bit != 0 {
            wire |= mapped;
        }
    }
    wire
}

#[cfg(test)]
mod tests {
    use super::toolkit_modifiers_to_wire;

    #[test]
    fn modifier_conversion_includes_the_unencodable_full_mask() {
        for input in 0u32..=255 {
            let wire = toolkit_modifiers_to_wire(input);
            assert_eq!(
                u32::from(wire),
                (input & 0x39) | ((input & 2) << 1) | ((input & 4) >> 1)
            );
        }
    }
}
