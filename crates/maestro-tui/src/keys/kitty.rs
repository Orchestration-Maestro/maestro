//! The enhanced keyboard reports: CSI-u, arrow, home/end and tilde sequences, and the xterm
//! modifyOtherKeys form.

use super::KeyEventType;
use super::names::{Code, KeyId, LOCKS, Named, shift_lowered, with_modifiers};

/// A cursor over the fields of a report.
struct Cursor<'a>(&'a str);

impl<'a> Cursor<'a> {
    /// Takes `literal` when the report continues with it.
    fn take(&mut self, literal: &str) -> Option<()> {
        self.0 = self.0.strip_prefix(literal)?;
        Some(())
    }

    /// Takes a run of ASCII digits, which may be empty.
    fn digits(&mut self) -> &'a str {
        let length = self.0.bytes().take_while(u8::is_ascii_digit).count();
        let (digits, rest) = self.0.split_at(length);
        self.0 = rest;
        digits
    }

    /// Takes `separator` and the digits after it, unless no digit follows.
    fn field(&mut self, separator: char) -> Option<&'a str> {
        let rest = self.0.strip_prefix(separator)?;
        let length = rest.bytes().take_while(u8::is_ascii_digit).count();
        if length == 0 {
            return None;
        }
        self.0 = &rest[length..];
        Some(&rest[..length])
    }

    /// Takes `separator` and any digits after it, which may be none.
    fn optional_field(&mut self, separator: char) -> Option<&'a str> {
        self.0 = self.0.strip_prefix(separator)?;
        Some(self.digits())
    }
}

/// How a report names its key.
enum Form {
    /// `CSI code u`: the first field is a Unicode code point.
    CodePoint,
    /// `CSI number ~`: the first field numbers a navigation key.
    Tilde,
    /// `CSI 1 ; modifier A` and its siblings: the final byte names the key.
    Final(Named),
}

/// The fields of one enhanced key report as written, before any value is checked.
struct Report<'a> {
    /// How the report names its key.
    form: Form,
    /// The code point of a CSI-u report or the key number of a tilde report; empty otherwise.
    first: &'a str,
    /// The code point the key produces with shift held.
    shifted: Option<&'a str>,
    /// The code point the key has on a standard layout.
    base: Option<&'a str>,
    /// The modifier field, one more than the modifier bits.
    modifier: Option<&'a str>,
    /// The event field.
    event: Option<&'a str>,
}

impl<'a> Report<'a> {
    /// Splits `data` into the fields of a complete report, or `None` when it is not one.
    fn scan(data: &'a str) -> Option<Self> {
        let body = data.strip_prefix("\x1b[")?;
        let end = body.chars().next_back()?;
        let mut cursor = Cursor(body.strip_suffix(end)?);
        let form = match end {
            'u' => Form::CodePoint,
            '~' => Form::Tilde,
            'A' => Form::Final(Named::Up),
            'B' => Form::Final(Named::Down),
            'C' => Form::Final(Named::Right),
            'D' => Form::Final(Named::Left),
            'H' => Form::Final(Named::Home),
            'F' => Form::Final(Named::End),
            _ => return None,
        };
        let mut report = Self {
            form,
            first: "",
            shifted: None,
            base: None,
            modifier: None,
            event: None,
        };
        match report.form {
            Form::CodePoint => {
                report.first = Some(cursor.digits()).filter(|digits| !digits.is_empty())?;
                report.shifted = cursor
                    .optional_field(':')
                    .filter(|digits| !digits.is_empty());
                report.base = cursor.field(':');
                report.modifier = cursor.field(';');
                report.event = cursor.field(':');
            }
            Form::Tilde => {
                report.first = Some(cursor.digits()).filter(|digits| !digits.is_empty())?;
                report.modifier = cursor.field(';');
                report.event = cursor.field(':');
            }
            Form::Final(_) => {
                cursor.take("1")?;
                report.modifier = Some(cursor.field(';')?);
                report.event = cursor.field(':');
            }
        }
        cursor.0.is_empty().then_some(report)
    }

    /// The phase the event field reports; absent or unknown numbers mean a press.
    fn event_type(&self) -> KeyEventType {
        match self.event.map(|digits| digits.trim_start_matches('0')) {
            Some("2") => KeyEventType::Repeat,
            Some("3") => KeyEventType::Release,
            _ => KeyEventType::Press,
        }
    }
}

/// Reads an unsigned decimal field of ASCII digits, rejecting overflow.
fn decimal(field: &str) -> Option<u32> {
    if field.is_empty() || !field.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    field.parse().ok()
}

/// Reads a code point field, rejecting surrogates and values beyond Unicode.
fn scalar(field: &str) -> Option<char> {
    char::from_u32(decimal(field)?)
}

/// The navigation key a tilde report numbers.
const fn tilde_key(number: u32) -> Option<Named> {
    match number {
        2 => Some(Named::Insert),
        3 => Some(Named::Delete),
        5 => Some(Named::PageUp),
        6 => Some(Named::PageDown),
        7 => Some(Named::Home),
        8 => Some(Named::End),
        _ => None,
    }
}

/// The phase of `data` when it is one complete enhanced report.
pub(super) fn event_type(data: &str) -> Option<KeyEventType> {
    Report::scan(data).map(|report| report.event_type())
}

/// Names a logical key held with `modifier`; a base-layout key stands in for an unrecognized key.
fn format_key(code: Code, modifier: u32, base: Option<char>) -> Option<KeyId> {
    let identity = code.identity(modifier);
    let effective = match base {
        Some(base) if !identity.is_authoritative() => Code::Char(base),
        _ => identity,
    };
    with_modifiers(&effective.name()?, modifier)
}

/// A key the terminal reported with an enhanced sequence.
pub(super) struct Sequence {
    /// The logical key as written, before keypad codes are mapped.
    pub(super) code: Code,
    /// The base-layout key of the physical key.
    pub(super) base: Option<char>,
    /// The key with shift held, when the terminal says so.
    pub(super) shifted: Option<char>,
    /// The modifier bits held, lock bits included.
    pub(super) modifier: u32,
}

impl Sequence {
    /// Reads a complete report that names a key; invalid numeric fields reject it.
    pub(super) fn parse(data: &str) -> Option<Self> {
        let report = Report::scan(data)?;
        let modifier = report.modifier.map_or(Some(1), decimal)?.checked_sub(1)?;
        let (code, shifted, base) = match report.form {
            Form::CodePoint => {
                let alternates = (report.shifted.map(scalar), report.base.map(scalar));
                if matches!(alternates, (Some(None), _) | (_, Some(None))) {
                    return None;
                }
                (
                    Code::Char(scalar(report.first)?),
                    alternates.0.flatten(),
                    alternates.1.flatten(),
                )
            }
            Form::Tilde => (Code::Key(tilde_key(decimal(report.first)?)?), None, None),
            Form::Final(key) => (Code::Key(key), None, None),
        };
        Some(Self {
            code,
            base,
            shifted,
            modifier,
        })
    }

    /// Whether the report is `expected` held with exactly `modifier`, ignoring lock bits.
    ///
    /// The base-layout key identifies the key only when the reported key is not a letter,
    /// digit or symbol, which are authoritative whatever the physical layout.
    pub(super) fn matches(&self, expected: Code, modifier: u32) -> bool {
        if self.modifier & !LOCKS != modifier & !LOCKS {
            return false;
        }
        let identity = self.code.identity(self.modifier);
        identity == expected
            || (!identity.is_authoritative()
                && self.base.is_some_and(|base| Code::Char(base) == expected))
    }

    /// The identifier of the key, absent for keys and modifiers without a name.
    pub(super) fn name(&self) -> Option<KeyId> {
        format_key(self.code, self.modifier, self.base)
    }
}

/// A key the terminal reported in the xterm modifyOtherKeys form.
#[derive(Clone, Copy)]
pub(super) struct ModifyOtherKeys {
    /// The key's code point.
    pub(super) key: char,
    /// The modifier bits held, lock bits included.
    pub(super) modifier: u32,
}

impl ModifyOtherKeys {
    /// Reads `CSI 27 ; modifier ; code ~`; invalid numeric fields reject it.
    pub(super) fn parse(data: &str) -> Option<Self> {
        let (modifier, key) = data
            .strip_prefix("\x1b[27;")?
            .strip_suffix('~')?
            .split_once(';')?;
        Some(Self {
            key: scalar(key)?,
            modifier: decimal(modifier)?.checked_sub(1)?,
        })
    }

    /// Whether the report is exactly `key` held with exactly `modifier`.
    pub(super) fn matches(self, key: char, modifier: u32) -> bool {
        self.key == key && self.modifier == modifier
    }

    /// Whether the report is `key` with the same non-empty modifiers, a shifted capital
    /// counting as its lowercase letter.
    pub(super) fn matches_printable(self, key: char, modifier: u32) -> bool {
        modifier != 0
            && self.modifier == modifier
            && shift_lowered(self.key, modifier) == shift_lowered(key, modifier)
    }

    /// The identifier of the key, absent for keys and modifiers without a name.
    pub(super) fn name(self) -> Option<KeyId> {
        format_key(Code::Char(self.key), self.modifier, None)
    }
}
