//! Legacy terminal input: the shared sequence table, raw control bytes and the Windows
//! Terminal backspace heuristic.

use super::names::{ALT, CTRL, KeyId, Named, SHIFT, with_modifiers};
use Named::{Clear, Delete, Down, End, Function, Home, Insert, Left, PageDown, PageUp, Right, Up};

/// The sequences a terminal sends for one key held with one modifier set.
struct Row {
    /// The key the sequences report.
    key: Named,
    /// Exactly the modifier bits held.
    modifier: u32,
    /// Whether the sequences are ambiguous once the enhanced protocol is on.
    legacy_only: bool,
    /// The sequences, each a complete input.
    sequences: &'static [&'static str],
}

/// A row that holds in both protocol states.
const fn row(key: Named, modifier: u32, sequences: &'static [&'static str]) -> Row {
    Row {
        key,
        modifier,
        legacy_only: false,
        sequences,
    }
}

/// Every legacy sequence with the key and modifiers it reports; matching and naming both read it.
const ROWS: [Row; 51] = [
    row(Up, 0, &["\x1b[A", "\x1bOA"]),
    row(Down, 0, &["\x1b[B", "\x1bOB"]),
    row(Right, 0, &["\x1b[C", "\x1bOC"]),
    row(Left, 0, &["\x1b[D", "\x1bOD"]),
    row(Home, 0, &["\x1b[H", "\x1bOH", "\x1b[1~", "\x1b[7~"]),
    row(End, 0, &["\x1b[F", "\x1bOF", "\x1b[4~", "\x1b[8~"]),
    row(Insert, 0, &["\x1b[2~"]),
    row(Delete, 0, &["\x1b[3~"]),
    row(PageUp, 0, &["\x1b[5~", "\x1b[[5~"]),
    row(PageDown, 0, &["\x1b[6~", "\x1b[[6~"]),
    row(Clear, 0, &["\x1b[E", "\x1bOE"]),
    row(Function(1), 0, &["\x1bOP", "\x1b[11~", "\x1b[[A"]),
    row(Function(2), 0, &["\x1bOQ", "\x1b[12~", "\x1b[[B"]),
    row(Function(3), 0, &["\x1bOR", "\x1b[13~", "\x1b[[C"]),
    row(Function(4), 0, &["\x1bOS", "\x1b[14~", "\x1b[[D"]),
    row(Function(5), 0, &["\x1b[15~", "\x1b[[E"]),
    row(Function(6), 0, &["\x1b[17~"]),
    row(Function(7), 0, &["\x1b[18~"]),
    row(Function(8), 0, &["\x1b[19~"]),
    row(Function(9), 0, &["\x1b[20~"]),
    row(Function(10), 0, &["\x1b[21~"]),
    row(Function(11), 0, &["\x1b[23~"]),
    row(Function(12), 0, &["\x1b[24~"]),
    row(Up, SHIFT, &["\x1b[a"]),
    row(Down, SHIFT, &["\x1b[b"]),
    row(Right, SHIFT, &["\x1b[c"]),
    row(Left, SHIFT, &["\x1b[d"]),
    row(Clear, SHIFT, &["\x1b[e"]),
    row(Insert, SHIFT, &["\x1b[2$"]),
    row(Delete, SHIFT, &["\x1b[3$"]),
    row(PageUp, SHIFT, &["\x1b[5$"]),
    row(PageDown, SHIFT, &["\x1b[6$"]),
    row(Home, SHIFT, &["\x1b[7$"]),
    row(End, SHIFT, &["\x1b[8$"]),
    row(Up, CTRL, &["\x1bOa"]),
    row(Down, CTRL, &["\x1bOb"]),
    row(Right, CTRL, &["\x1bOc"]),
    row(Left, CTRL, &["\x1bOd"]),
    row(Clear, CTRL, &["\x1bOe"]),
    row(Insert, CTRL, &["\x1b[2^"]),
    row(Delete, CTRL, &["\x1b[3^"]),
    row(PageUp, CTRL, &["\x1b[5^"]),
    row(PageDown, CTRL, &["\x1b[6^"]),
    row(Home, CTRL, &["\x1b[7^"]),
    row(End, CTRL, &["\x1b[8^"]),
    row(Up, ALT, &["\x1bp"]),
    row(Down, ALT, &["\x1bn"]),
    row(Left, ALT, &["\x1bb"]),
    row(Right, ALT, &["\x1bf"]),
    Row {
        legacy_only: true,
        ..row(Left, ALT, &["\x1bB"])
    },
    Row {
        legacy_only: true,
        ..row(Right, ALT, &["\x1bF"])
    },
];

impl Row {
    /// Whether the row's sequences are in use for the given protocol state.
    const fn applies(&self, kitty: bool) -> bool {
        !(self.legacy_only && kitty)
    }
}

/// Whether `data` is a sequence of `key` held with exactly `modifier`.
pub(super) fn matches(data: &str, key: Named, modifier: u32, kitty: bool) -> bool {
    ROWS.iter().any(|row| {
        row.key == key
            && row.modifier == modifier
            && row.applies(kitty)
            && row.sequences.contains(&data)
    })
}

/// The identifier of a sequence in the table.
fn identify(data: &str, kitty: bool) -> Option<KeyId> {
    let row = ROWS
        .iter()
        .find(|row| row.applies(kitty) && row.sequences.contains(&data))?;
    with_modifiers(&row.key.name(), row.modifier)
}

/// Whether the session is a local Windows Terminal, where a raw backspace byte is ctrl+backspace.
fn windows_terminal_session() -> bool {
    let set = |name| std::env::var_os(name).is_some_and(|value| !value.is_empty());
    set("WT_SESSION")
        && !["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"]
            .into_iter()
            .any(set)
}

/// Whether `data` is a raw backspace byte meaning backspace held with exactly `modifier`.
///
/// DEL is plain backspace. Byte 8 is plain backspace except in a local Windows Terminal,
/// where it is ctrl+backspace.
pub(super) fn matches_raw_backspace(data: &str, modifier: u32) -> bool {
    match data {
        "\x7f" => modifier == 0,
        "\x08" => modifier == if windows_terminal_session() { CTRL } else { 0 },
        _ => false,
    }
}

/// Names the key reported by a legacy sequence or raw control byte.
pub(super) fn parse(data: &str, kitty: bool) -> Option<KeyId> {
    if kitty && matches!(data, "\x1b\r" | "\n") {
        return Some("shift+enter".to_owned());
    }
    if let Some(id) = identify(data, kitty) {
        return Some(id);
    }
    let fixed = match data {
        "\x1b" => "escape",
        "\x1c" => "ctrl+\\",
        "\x1d" => "ctrl+]",
        "\x1f" => "ctrl+-",
        "\x1b\x1b" => "ctrl+alt+[",
        "\x1b\x1c" => "ctrl+alt+\\",
        "\x1b\x1d" => "ctrl+alt+]",
        "\x1b\x1f" => "ctrl+alt+-",
        "\t" => "tab",
        "\r" | "\n" | "\x1bOM" => "enter",
        "\0" => "ctrl+space",
        " " => "space",
        "\x08" if windows_terminal_session() => "ctrl+backspace",
        "\x7f" | "\x08" => "backspace",
        "\x1b[Z" => "shift+tab",
        "\x1b\r" => "alt+enter",
        "\x1b " if !kitty => "alt+space",
        "\x1b\x7f" | "\x1b\x08" => "alt+backspace",
        _ => return prefixed_or_raw(data, kitty),
    };
    Some(fixed.to_owned())
}

/// Names an escape-prefixed or single raw byte: alt and ctrl letters, or the byte itself.
fn prefixed_or_raw(data: &str, kitty: bool) -> Option<KeyId> {
    match data.as_bytes() {
        [0x1b, code @ 1..=26] if !kitty => Some(format!("ctrl+alt+{}", char::from(code + 96))),
        [0x1b, code @ (b'a'..=b'z' | b'0'..=b'9')] if !kitty => {
            Some(format!("alt+{}", char::from(*code)))
        }
        [code @ 1..=26] => Some(format!("ctrl+{}", char::from(code + 96))),
        [code @ 32..=126] => Some(char::from(*code).to_string()),
        _ => None,
    }
}
