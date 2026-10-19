//! Structured host admission over the shared keyboard report scanner.

use super::kitty::Report;
use super::names::SHIFT;
use super::{Code, KeyEventType, Sequence};

/// Reads one complete host key, with strict phases and optional associated text.
#[must_use]
pub fn parse_terminal_key_sequence(data: &str) -> Option<Sequence> {
    parse_report(data)
        .or_else(|| {
            let (modifier, point) = super::kitty::ModifyOtherKeys::fields(data, true)?;
            Some(Sequence::host(Code::host(point)?, modifier))
        })
        .or_else(|| super::legacy::host_character(data))
}

/// Decodes complete structured or legacy special reports.
fn parse_report(data: &str) -> Option<Sequence> {
    if data == "\x1b[Z" {
        return Some(Sequence::host(Code::Key(super::Named::BackTab), SHIFT));
    }
    if let Some(key) = data
        .strip_prefix('\x1b')
        .and_then(super::legacy::host_special)
        && matches!(
            key,
            super::Named::Up | super::Named::Down | super::Named::Left | super::Named::Right
        )
        && data.starts_with("\x1b\x1b[")
    {
        return Some(Sequence::host(Code::Key(key), super::names::ALT));
    }
    if let Some(key) = super::legacy::host_special(data) {
        return Some(Sequence::host(Code::Key(key), 0));
    }
    if let Some(code) = super::legacy::host_keypad(data) {
        return Some(Sequence::host(code, 0));
    }
    let report = Report::scan(data, true)?;
    let event_type = match report.event.unwrap_or("1") {
        "1" => KeyEventType::Press,
        "2" => KeyEventType::Repeat,
        "3" => KeyEventType::Release,
        _ => return None,
    };
    let mut sequence = Sequence::from_report(&report, true)?;
    sequence.event_type = event_type;
    sequence.generated_text = match report.associated {
        Some(text) => Some(associated_text(text)?),
        None => None,
    };
    if matches!(sequence.code, Code::Char(_))
        && sequence.shifted.is_some_and(|shifted| {
            Some(shifted) != report.codepoint() && char::from_u32(shifted).is_some()
        })
    {
        sequence.modifier |= SHIFT;
    }
    Some(sequence)
}

/// Decodes nonempty associated text, rejecting controls and invalid Unicode scalars.
fn associated_text(value: &str) -> Option<String> {
    value.split(':').try_fold(String::new(), |mut text, value| {
        let ch = char::from_u32(value.parse().ok()?)?;
        if ch.is_control() {
            return None;
        }
        text.push(ch);
        Some(text)
    })
}
