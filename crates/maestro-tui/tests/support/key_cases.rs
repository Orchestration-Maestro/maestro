//! Key sequences with the exact result of every decoding operation, grouped by what they probe.
//!
//! A table lists, for each input, the key identifier it parses to and the candidate
//! identifiers it matches; every other candidate of the table must not match. The parsed
//! identifier is tried as a candidate too when the table does not list it.

use maestro_tui::KeyEventType;

/// The keyboard protocol states a row is observed in.
#[derive(Clone, Copy, Debug)]
pub enum Protocol {
    /// Only while the enhanced protocol is off.
    Legacy,
    /// Only while the enhanced protocol is on.
    Enhanced,
    /// In both states, with the same result.
    Both,
}

/// One input and the exact result of every operation on it.
#[derive(Clone, Copy, Debug)]
pub struct Row {
    /// Protocol states the row holds in.
    pub protocol: Protocol,
    /// The raw terminal input.
    pub data: &'static str,
    /// The identifier the input parses to.
    pub parsed: Option<&'static str>,
    /// The candidate identifiers the input matches.
    pub matches: &'static [&'static str],
    /// The text the enhanced decoder returns.
    pub kitty_text: Option<char>,
    /// The text either decoder returns.
    pub text: Option<char>,
    /// The phase the input reports.
    pub event: KeyEventType,
}

impl Row {
    /// A row that decodes no text and reports a press.
    pub const fn new(
        protocol: Protocol,
        data: &'static str,
        parsed: Option<&'static str>,
        matches: &'static [&'static str],
    ) -> Self {
        Self {
            protocol,
            data,
            parsed,
            matches,
            kitty_text: None,
            text: None,
            event: KeyEventType::Press,
        }
    }
}

use Protocol::{Both, Enhanced, Legacy};

/// Candidate identifiers tried against every row of [`RAW`].
pub const CAND: &[&str] = &[
    "escape",
    "esc",
    "enter",
    "return",
    "tab",
    "space",
    "backspace",
    "ctrl+backspace",
    "ctrl+h",
    "ctrl+space",
    "shift+enter",
    "alt+enter",
    "shift+tab",
    "up",
    "down",
    "left",
    "right",
    "home",
    "end",
    "insert",
    "delete",
    "pageUp",
    "pageDown",
    "clear",
    "f1",
    "f12",
    "a",
    "shift+a",
    "ctrl+a",
    "alt+a",
    "ctrl+alt+a",
];

/// Rows of the `raw` probe.
pub const RAW: &[Row] = &[
    Row::new(Legacy, "\0", Some("ctrl+space"), &["ctrl+space"]),
    Row::new(Enhanced, "\0", Some("ctrl+space"), &[]),
    Row::new(Both, "\x01", Some("ctrl+a"), &["ctrl+a"]),
    Row::new(Both, "\x02", Some("ctrl+b"), &["ctrl+b"]),
    Row::new(Both, "\x03", Some("ctrl+c"), &["ctrl+c"]),
    Row::new(Both, "\x04", Some("ctrl+d"), &["ctrl+d"]),
    Row::new(Both, "\x05", Some("ctrl+e"), &["ctrl+e"]),
    Row::new(Both, "\x06", Some("ctrl+f"), &["ctrl+f"]),
    Row::new(Both, "\x07", Some("ctrl+g"), &["ctrl+g"]),
    Row::new(Both, "\x08", Some("backspace"), &["backspace", "ctrl+h"]),
    Row::new(Both, "\t", Some("tab"), &["tab"]),
    Row::new(Legacy, "\n", Some("enter"), &["enter", "return"]),
    Row::new(Enhanced, "\n", Some("shift+enter"), &["shift+enter"]),
    Row::new(Both, "\x0b", Some("ctrl+k"), &["ctrl+k"]),
    Row::new(Both, "\x0c", Some("ctrl+l"), &["ctrl+l"]),
    Row::new(Both, "\r", Some("enter"), &["enter", "return"]),
    Row::new(Both, "\x0e", Some("ctrl+n"), &["ctrl+n"]),
    Row::new(Both, "\x0f", Some("ctrl+o"), &["ctrl+o"]),
    Row::new(Both, "\x10", Some("ctrl+p"), &["ctrl+p"]),
    Row::new(Both, "\x11", Some("ctrl+q"), &["ctrl+q"]),
    Row::new(Both, "\x12", Some("ctrl+r"), &["ctrl+r"]),
    Row::new(Both, "\x13", Some("ctrl+s"), &["ctrl+s"]),
    Row::new(Both, "\x14", Some("ctrl+t"), &["ctrl+t"]),
    Row::new(Both, "\x15", Some("ctrl+u"), &["ctrl+u"]),
    Row::new(Both, "\x16", Some("ctrl+v"), &["ctrl+v"]),
    Row::new(Both, "\x17", Some("ctrl+w"), &["ctrl+w"]),
    Row::new(Both, "\x18", Some("ctrl+x"), &["ctrl+x"]),
    Row::new(Both, "\x19", Some("ctrl+y"), &["ctrl+y"]),
    Row::new(Both, "\x1a", Some("ctrl+z"), &["ctrl+z"]),
    Row::new(Both, "\x1b", Some("escape"), &["escape", "esc"]),
    Row::new(Both, "\x1c", Some("ctrl+\\"), &["ctrl+\\"]),
    Row::new(Both, "\x1d", Some("ctrl+]"), &["ctrl+]"]),
    Row::new(Both, "\x1e", None, &[]),
    Row::new(Both, "\x1f", Some("ctrl+-"), &["ctrl+-"]),
    Row::new(Both, " ", Some("space"), &["space"]),
    Row::new(Both, "!", Some("!"), &["!"]),
    Row::new(Both, "\"", Some("\""), &[]),
    Row::new(Both, "#", Some("#"), &["#"]),
    Row::new(Both, "$", Some("$"), &["$"]),
    Row::new(Both, "%", Some("%"), &["%"]),
    Row::new(Both, "&", Some("&"), &["&"]),
    Row::new(Both, "'", Some("'"), &["'"]),
    Row::new(Both, "(", Some("("), &["("]),
    Row::new(Both, ")", Some(")"), &[")"]),
    Row::new(Both, "*", Some("*"), &["*"]),
    Row::new(Both, ",", Some(","), &[","]),
    Row::new(Both, "-", Some("-"), &["-"]),
    Row::new(Both, ".", Some("."), &["."]),
    Row::new(Both, "/", Some("/"), &["/"]),
    Row::new(Both, "0", Some("0"), &["0"]),
    Row::new(Both, "1", Some("1"), &["1"]),
    Row::new(Both, "2", Some("2"), &["2"]),
    Row::new(Both, "3", Some("3"), &["3"]),
    Row::new(Both, "4", Some("4"), &["4"]),
    Row::new(Both, "5", Some("5"), &["5"]),
    Row::new(Both, "6", Some("6"), &["6"]),
    Row::new(Both, "7", Some("7"), &["7"]),
    Row::new(Both, "8", Some("8"), &["8"]),
    Row::new(Both, "9", Some("9"), &["9"]),
    Row::new(Both, ":", Some(":"), &[":"]),
    Row::new(Both, ";", Some(";"), &[";"]),
    Row::new(Both, "<", Some("<"), &["<"]),
    Row::new(Both, "=", Some("="), &["="]),
    Row::new(Both, ">", Some(">"), &[">"]),
    Row::new(Both, "?", Some("?"), &["?"]),
    Row::new(Both, "@", Some("@"), &["@"]),
    Row::new(Both, "A", Some("A"), &["shift+a"]),
    Row::new(Both, "B", Some("B"), &[]),
    Row::new(Both, "C", Some("C"), &[]),
    Row::new(Both, "D", Some("D"), &[]),
    Row::new(Both, "E", Some("E"), &[]),
    Row::new(Both, "F", Some("F"), &[]),
    Row::new(Both, "G", Some("G"), &[]),
    Row::new(Both, "H", Some("H"), &[]),
    Row::new(Both, "I", Some("I"), &[]),
    Row::new(Both, "J", Some("J"), &[]),
    Row::new(Both, "K", Some("K"), &[]),
    Row::new(Both, "L", Some("L"), &[]),
    Row::new(Both, "M", Some("M"), &[]),
    Row::new(Both, "N", Some("N"), &[]),
    Row::new(Both, "O", Some("O"), &[]),
    Row::new(Both, "P", Some("P"), &[]),
    Row::new(Both, "Q", Some("Q"), &[]),
    Row::new(Both, "R", Some("R"), &[]),
    Row::new(Both, "S", Some("S"), &[]),
    Row::new(Both, "T", Some("T"), &[]),
    Row::new(Both, "U", Some("U"), &[]),
    Row::new(Both, "V", Some("V"), &[]),
    Row::new(Both, "W", Some("W"), &[]),
    Row::new(Both, "X", Some("X"), &[]),
    Row::new(Both, "Y", Some("Y"), &[]),
    Row::new(Both, "Z", Some("Z"), &[]),
    Row::new(Both, "[", Some("["), &["["]),
    Row::new(Both, "\\", Some("\\"), &["\\"]),
    Row::new(Both, "]", Some("]"), &["]"]),
    Row::new(Both, "^", Some("^"), &["^"]),
    Row::new(Both, "_", Some("_"), &["_"]),
    Row::new(Both, "`", Some("`"), &["`"]),
    Row::new(Both, "a", Some("a"), &["a"]),
    Row::new(Both, "b", Some("b"), &["b"]),
    Row::new(Both, "c", Some("c"), &["c"]),
    Row::new(Both, "d", Some("d"), &["d"]),
    Row::new(Both, "e", Some("e"), &["e"]),
    Row::new(Both, "f", Some("f"), &["f"]),
    Row::new(Both, "g", Some("g"), &["g"]),
    Row::new(Both, "h", Some("h"), &["h"]),
    Row::new(Both, "i", Some("i"), &["i"]),
    Row::new(Both, "j", Some("j"), &["j"]),
    Row::new(Both, "k", Some("k"), &["k"]),
    Row::new(Both, "l", Some("l"), &["l"]),
    Row::new(Both, "m", Some("m"), &["m"]),
    Row::new(Both, "n", Some("n"), &["n"]),
    Row::new(Both, "o", Some("o"), &["o"]),
    Row::new(Both, "p", Some("p"), &["p"]),
    Row::new(Both, "q", Some("q"), &["q"]),
    Row::new(Both, "r", Some("r"), &["r"]),
    Row::new(Both, "s", Some("s"), &["s"]),
    Row::new(Both, "t", Some("t"), &["t"]),
    Row::new(Both, "u", Some("u"), &["u"]),
    Row::new(Both, "v", Some("v"), &["v"]),
    Row::new(Both, "w", Some("w"), &["w"]),
    Row::new(Both, "x", Some("x"), &["x"]),
    Row::new(Both, "y", Some("y"), &["y"]),
    Row::new(Both, "z", Some("z"), &["z"]),
    Row::new(Both, "{", Some("{"), &["{"]),
    Row::new(Both, "|", Some("|"), &["|"]),
    Row::new(Both, "}", Some("}"), &["}"]),
    Row::new(Both, "~", Some("~"), &["~"]),
    Row::new(Both, "\x7f", Some("backspace"), &["backspace"]),
];

/// Candidate identifiers tried against every row of [`LEGACY`].
pub const LEGACY_CAND: &[&str] = &[
    "escape",
    "esc",
    "enter",
    "return",
    "tab",
    "space",
    "backspace",
    "ctrl+backspace",
    "ctrl+h",
    "ctrl+space",
    "shift+enter",
    "alt+enter",
    "shift+tab",
    "up",
    "down",
    "left",
    "right",
    "home",
    "end",
    "insert",
    "delete",
    "pageUp",
    "pageDown",
    "clear",
    "f1",
    "f12",
    "a",
    "shift+a",
    "ctrl+a",
    "alt+a",
    "ctrl+alt+a",
    "shift+up",
    "ctrl+up",
    "shift+insert",
    "ctrl+insert",
    "alt+left",
    "alt+right",
    "alt+up",
    "alt+down",
];

/// Rows of the `legacy` probe.
pub const LEGACY: &[Row] = &[
    Row::new(Both, "\x1b[A", Some("up"), &["up"]),
    Row::new(Both, "\x1bOA", Some("up"), &["up"]),
    Row::new(Both, "\x1b[B", Some("down"), &["down"]),
    Row::new(Both, "\x1bOB", Some("down"), &["down"]),
    Row::new(Both, "\x1b[C", Some("right"), &["right"]),
    Row::new(Both, "\x1bOC", Some("right"), &["right"]),
    Row::new(Both, "\x1b[D", Some("left"), &["left"]),
    Row::new(Both, "\x1bOD", Some("left"), &["left"]),
    Row::new(Both, "\x1b[H", Some("home"), &["home"]),
    Row::new(Both, "\x1bOH", Some("home"), &["home"]),
    Row::new(Both, "\x1b[1~", Some("home"), &["home"]),
    Row::new(Both, "\x1b[7~", Some("home"), &["home"]),
    Row::new(Both, "\x1b[F", Some("end"), &["end"]),
    Row::new(Both, "\x1bOF", Some("end"), &["end"]),
    Row::new(Both, "\x1b[4~", Some("end"), &["end"]),
    Row::new(Both, "\x1b[8~", Some("end"), &["end"]),
    Row::new(Both, "\x1b[2~", Some("insert"), &["insert"]),
    Row::new(Both, "\x1b[3~", Some("delete"), &["delete"]),
    Row::new(Both, "\x1b[5~", Some("pageUp"), &["pageUp"]),
    Row::new(Both, "\x1b[[5~", Some("pageUp"), &["pageUp"]),
    Row::new(Both, "\x1b[6~", Some("pageDown"), &["pageDown"]),
    Row::new(Both, "\x1b[[6~", Some("pageDown"), &["pageDown"]),
    Row::new(Both, "\x1b[E", Some("clear"), &["clear"]),
    Row::new(Both, "\x1bOE", Some("clear"), &["clear"]),
    Row::new(Both, "\x1bOP", Some("f1"), &["f1"]),
    Row::new(Both, "\x1b[11~", Some("f1"), &["f1"]),
    Row::new(Both, "\x1b[[A", Some("f1"), &["f1"]),
    Row::new(Both, "\x1bOQ", Some("f2"), &["f2"]),
    Row::new(Both, "\x1b[12~", Some("f2"), &["f2"]),
    Row::new(Both, "\x1b[[B", Some("f2"), &["f2"]),
    Row::new(Both, "\x1bOR", Some("f3"), &["f3"]),
    Row::new(Both, "\x1b[13~", Some("f3"), &["f3"]),
    Row::new(Both, "\x1b[[C", Some("f3"), &["f3"]),
    Row::new(Both, "\x1bOS", Some("f4"), &["f4"]),
    Row::new(Both, "\x1b[14~", Some("f4"), &["f4"]),
    Row::new(Both, "\x1b[[D", Some("f4"), &["f4"]),
    Row::new(Both, "\x1b[15~", Some("f5"), &["f5"]),
    Row::new(Both, "\x1b[[E", Some("f5"), &["f5"]),
    Row::new(Both, "\x1b[17~", Some("f6"), &["f6"]),
    Row::new(Both, "\x1b[18~", Some("f7"), &["f7"]),
    Row::new(Both, "\x1b[19~", Some("f8"), &["f8"]),
    Row::new(Both, "\x1b[20~", Some("f9"), &["f9"]),
    Row::new(Both, "\x1b[21~", Some("f10"), &["f10"]),
    Row::new(Both, "\x1b[23~", Some("f11"), &["f11"]),
    Row::new(Both, "\x1b[24~", Some("f12"), &["f12"]),
    Row::new(Both, "\x1b[a", Some("shift+up"), &["shift+up"]),
    Row::new(Both, "\x1b[b", Some("shift+down"), &["shift+down"]),
    Row::new(Both, "\x1b[c", Some("shift+right"), &["shift+right"]),
    Row::new(Both, "\x1b[d", Some("shift+left"), &["shift+left"]),
    Row::new(Both, "\x1b[e", Some("shift+clear"), &["shift+clear"]),
    Row::new(Both, "\x1b[2$", Some("shift+insert"), &["shift+insert"]),
    Row::new(Both, "\x1b[3$", Some("shift+delete"), &["shift+delete"]),
    Row::new(Both, "\x1b[5$", Some("shift+pageUp"), &["shift+pageUp"]),
    Row::new(Both, "\x1b[6$", Some("shift+pageDown"), &["shift+pageDown"]),
    Row::new(Both, "\x1b[7$", Some("shift+home"), &["shift+home"]),
    Row::new(Both, "\x1b[8$", Some("shift+end"), &["shift+end"]),
    Row::new(Both, "\x1bOa", Some("ctrl+up"), &["ctrl+up"]),
    Row::new(Both, "\x1bOb", Some("ctrl+down"), &["ctrl+down"]),
    Row::new(Both, "\x1bOc", Some("ctrl+right"), &["ctrl+right"]),
    Row::new(Both, "\x1bOd", Some("ctrl+left"), &["ctrl+left"]),
    Row::new(Both, "\x1bOe", Some("ctrl+clear"), &["ctrl+clear"]),
    Row::new(Both, "\x1b[2^", Some("ctrl+insert"), &["ctrl+insert"]),
    Row::new(Both, "\x1b[3^", Some("ctrl+delete"), &["ctrl+delete"]),
    Row::new(Both, "\x1b[5^", Some("ctrl+pageUp"), &["ctrl+pageUp"]),
    Row::new(Both, "\x1b[6^", Some("ctrl+pageDown"), &["ctrl+pageDown"]),
    Row::new(Both, "\x1b[7^", Some("ctrl+home"), &["ctrl+home"]),
    Row::new(Both, "\x1b[8^", Some("ctrl+end"), &["ctrl+end"]),
    Row::new(Both, "\x1bb", Some("alt+left"), &["alt+left"]),
    Row::new(Both, "\x1bf", Some("alt+right"), &["alt+right"]),
    Row::new(Both, "\x1bp", Some("alt+up"), &["alt+up"]),
    Row::new(Both, "\x1bn", Some("alt+down"), &["alt+down"]),
];

/// Candidate identifiers tried against every row of [`ESCAPE`].
pub const ESCAPE_CAND: &[&str] = &[
    "escape",
    "esc",
    "enter",
    "return",
    "tab",
    "space",
    "backspace",
    "ctrl+backspace",
    "ctrl+h",
    "ctrl+space",
    "shift+enter",
    "alt+enter",
    "shift+tab",
    "up",
    "down",
    "left",
    "right",
    "home",
    "end",
    "insert",
    "delete",
    "pageUp",
    "pageDown",
    "clear",
    "f1",
    "f12",
    "a",
    "shift+a",
    "ctrl+a",
    "alt+a",
    "ctrl+alt+a",
    "ctrl+alt+[",
    "ctrl+alt+\\",
    "ctrl+alt+]",
    "ctrl+alt+-",
    "alt+space",
    "alt+backspace",
    "alt+left",
    "alt+right",
    "alt+b",
    "alt+f",
    "alt+n",
    "alt+p",
];

/// Rows of the `escape` probe.
pub const ESCAPE: &[Row] = &[
    Row::new(Both, "\x1b\0", None, &[]),
    Row::new(Legacy, "\x1b\x01", Some("ctrl+alt+a"), &["ctrl+alt+a"]),
    Row::new(Enhanced, "\x1b\x01", None, &[]),
    Row::new(Legacy, "\x1b\x02", Some("ctrl+alt+b"), &["ctrl+alt+b"]),
    Row::new(Enhanced, "\x1b\x02", None, &[]),
    Row::new(Legacy, "\x1b\x03", Some("ctrl+alt+c"), &["ctrl+alt+c"]),
    Row::new(Enhanced, "\x1b\x03", None, &[]),
    Row::new(Legacy, "\x1b\x04", Some("ctrl+alt+d"), &["ctrl+alt+d"]),
    Row::new(Enhanced, "\x1b\x04", None, &[]),
    Row::new(Legacy, "\x1b\x05", Some("ctrl+alt+e"), &["ctrl+alt+e"]),
    Row::new(Enhanced, "\x1b\x05", None, &[]),
    Row::new(Legacy, "\x1b\x06", Some("ctrl+alt+f"), &["ctrl+alt+f"]),
    Row::new(Enhanced, "\x1b\x06", None, &[]),
    Row::new(Legacy, "\x1b\x07", Some("ctrl+alt+g"), &["ctrl+alt+g"]),
    Row::new(Enhanced, "\x1b\x07", None, &[]),
    Row::new(Both, "\x1b\x08", Some("alt+backspace"), &["alt+backspace"]),
    Row::new(Legacy, "\x1b\t", Some("ctrl+alt+i"), &["ctrl+alt+i"]),
    Row::new(Enhanced, "\x1b\t", None, &[]),
    Row::new(Legacy, "\x1b\n", Some("ctrl+alt+j"), &["ctrl+alt+j"]),
    Row::new(Enhanced, "\x1b\n", None, &[]),
    Row::new(Legacy, "\x1b\x0b", Some("ctrl+alt+k"), &["ctrl+alt+k"]),
    Row::new(Enhanced, "\x1b\x0b", None, &[]),
    Row::new(Legacy, "\x1b\x0c", Some("ctrl+alt+l"), &["ctrl+alt+l"]),
    Row::new(Enhanced, "\x1b\x0c", None, &[]),
    Row::new(Legacy, "\x1b\r", Some("alt+enter"), &["alt+enter"]),
    Row::new(Enhanced, "\x1b\r", Some("shift+enter"), &["shift+enter"]),
    Row::new(Legacy, "\x1b\x0e", Some("ctrl+alt+n"), &["ctrl+alt+n"]),
    Row::new(Enhanced, "\x1b\x0e", None, &[]),
    Row::new(Legacy, "\x1b\x0f", Some("ctrl+alt+o"), &["ctrl+alt+o"]),
    Row::new(Enhanced, "\x1b\x0f", None, &[]),
    Row::new(Legacy, "\x1b\x10", Some("ctrl+alt+p"), &["ctrl+alt+p"]),
    Row::new(Enhanced, "\x1b\x10", None, &[]),
    Row::new(Legacy, "\x1b\x11", Some("ctrl+alt+q"), &["ctrl+alt+q"]),
    Row::new(Enhanced, "\x1b\x11", None, &[]),
    Row::new(Legacy, "\x1b\x12", Some("ctrl+alt+r"), &["ctrl+alt+r"]),
    Row::new(Enhanced, "\x1b\x12", None, &[]),
    Row::new(Legacy, "\x1b\x13", Some("ctrl+alt+s"), &["ctrl+alt+s"]),
    Row::new(Enhanced, "\x1b\x13", None, &[]),
    Row::new(Legacy, "\x1b\x14", Some("ctrl+alt+t"), &["ctrl+alt+t"]),
    Row::new(Enhanced, "\x1b\x14", None, &[]),
    Row::new(Legacy, "\x1b\x15", Some("ctrl+alt+u"), &["ctrl+alt+u"]),
    Row::new(Enhanced, "\x1b\x15", None, &[]),
    Row::new(Legacy, "\x1b\x16", Some("ctrl+alt+v"), &["ctrl+alt+v"]),
    Row::new(Enhanced, "\x1b\x16", None, &[]),
    Row::new(Legacy, "\x1b\x17", Some("ctrl+alt+w"), &["ctrl+alt+w"]),
    Row::new(Enhanced, "\x1b\x17", None, &[]),
    Row::new(Legacy, "\x1b\x18", Some("ctrl+alt+x"), &["ctrl+alt+x"]),
    Row::new(Enhanced, "\x1b\x18", None, &[]),
    Row::new(Legacy, "\x1b\x19", Some("ctrl+alt+y"), &["ctrl+alt+y"]),
    Row::new(Enhanced, "\x1b\x19", None, &[]),
    Row::new(Legacy, "\x1b\x1a", Some("ctrl+alt+z"), &["ctrl+alt+z"]),
    Row::new(Enhanced, "\x1b\x1a", None, &[]),
    Row::new(Legacy, "\x1b\x1b", Some("ctrl+alt+["), &["ctrl+alt+["]),
    Row::new(Enhanced, "\x1b\x1b", Some("ctrl+alt+["), &[]),
    Row::new(Legacy, "\x1b\x1c", Some("ctrl+alt+\\"), &["ctrl+alt+\\"]),
    Row::new(Enhanced, "\x1b\x1c", Some("ctrl+alt+\\"), &[]),
    Row::new(Legacy, "\x1b\x1d", Some("ctrl+alt+]"), &["ctrl+alt+]"]),
    Row::new(Enhanced, "\x1b\x1d", Some("ctrl+alt+]"), &[]),
    Row::new(Both, "\x1b\x1e", None, &[]),
    Row::new(Legacy, "\x1b\x1f", Some("ctrl+alt+-"), &["ctrl+alt+-"]),
    Row::new(Enhanced, "\x1b\x1f", Some("ctrl+alt+-"), &[]),
    Row::new(Legacy, "\x1b ", Some("alt+space"), &["alt+space"]),
    Row::new(Enhanced, "\x1b ", None, &[]),
    Row::new(Both, "\x1b!", None, &[]),
    Row::new(Both, "\x1b\"", None, &[]),
    Row::new(Both, "\x1b#", None, &[]),
    Row::new(Both, "\x1b$", None, &[]),
    Row::new(Both, "\x1b%", None, &[]),
    Row::new(Both, "\x1b&", None, &[]),
    Row::new(Both, "\x1b'", None, &[]),
    Row::new(Both, "\x1b(", None, &[]),
    Row::new(Both, "\x1b)", None, &[]),
    Row::new(Both, "\x1b*", None, &[]),
    Row::new(Both, "\x1b+", None, &[]),
    Row::new(Both, "\x1b,", None, &[]),
    Row::new(Both, "\x1b-", None, &[]),
    Row::new(Both, "\x1b.", None, &[]),
    Row::new(Both, "\x1b/", None, &[]),
    Row::new(Legacy, "\x1b0", Some("alt+0"), &["alt+0"]),
    Row::new(Enhanced, "\x1b0", None, &[]),
    Row::new(Legacy, "\x1b1", Some("alt+1"), &["alt+1"]),
    Row::new(Enhanced, "\x1b1", None, &[]),
    Row::new(Legacy, "\x1b2", Some("alt+2"), &["alt+2"]),
    Row::new(Enhanced, "\x1b2", None, &[]),
    Row::new(Legacy, "\x1b3", Some("alt+3"), &["alt+3"]),
    Row::new(Enhanced, "\x1b3", None, &[]),
    Row::new(Legacy, "\x1b4", Some("alt+4"), &["alt+4"]),
    Row::new(Enhanced, "\x1b4", None, &[]),
    Row::new(Legacy, "\x1b5", Some("alt+5"), &["alt+5"]),
    Row::new(Enhanced, "\x1b5", None, &[]),
    Row::new(Legacy, "\x1b6", Some("alt+6"), &["alt+6"]),
    Row::new(Enhanced, "\x1b6", None, &[]),
    Row::new(Legacy, "\x1b7", Some("alt+7"), &["alt+7"]),
    Row::new(Enhanced, "\x1b7", None, &[]),
    Row::new(Legacy, "\x1b8", Some("alt+8"), &["alt+8"]),
    Row::new(Enhanced, "\x1b8", None, &[]),
    Row::new(Legacy, "\x1b9", Some("alt+9"), &["alt+9"]),
    Row::new(Enhanced, "\x1b9", None, &[]),
    Row::new(Both, "\x1b:", None, &[]),
    Row::new(Both, "\x1b;", None, &[]),
    Row::new(Both, "\x1b<", None, &[]),
    Row::new(Both, "\x1b=", None, &[]),
    Row::new(Both, "\x1b>", None, &[]),
    Row::new(Both, "\x1b?", None, &[]),
    Row::new(Both, "\x1b@", None, &[]),
    Row::new(Both, "\x1bA", None, &[]),
    Row::new(Legacy, "\x1bB", Some("alt+left"), &["alt+left"]),
    Row::new(Enhanced, "\x1bB", None, &[]),
    Row::new(Both, "\x1bC", None, &[]),
    Row::new(Both, "\x1bD", None, &[]),
    Row::new(Both, "\x1bE", None, &[]),
    Row::new(Legacy, "\x1bF", Some("alt+right"), &["alt+right"]),
    Row::new(Enhanced, "\x1bF", None, &[]),
    Row::new(Both, "\x1bG", None, &[]),
    Row::new(Both, "\x1bH", None, &[]),
    Row::new(Both, "\x1bI", None, &[]),
    Row::new(Both, "\x1bJ", None, &[]),
    Row::new(Both, "\x1bK", None, &[]),
    Row::new(Both, "\x1bL", None, &[]),
    Row::new(Both, "\x1bM", None, &[]),
    Row::new(Both, "\x1bN", None, &[]),
    Row::new(Both, "\x1bO", None, &[]),
    Row::new(Both, "\x1bP", None, &[]),
    Row::new(Both, "\x1bQ", None, &[]),
    Row::new(Both, "\x1bR", None, &[]),
    Row::new(Both, "\x1bS", None, &[]),
    Row::new(Both, "\x1bT", None, &[]),
    Row::new(Both, "\x1bU", None, &[]),
    Row::new(Both, "\x1bV", None, &[]),
    Row::new(Both, "\x1bW", None, &[]),
    Row::new(Both, "\x1bX", None, &[]),
    Row::new(Both, "\x1bY", None, &[]),
    Row::new(Both, "\x1bZ", None, &[]),
    Row::new(Both, "\x1b[", None, &[]),
    Row::new(Both, "\x1b\\", None, &[]),
    Row::new(Both, "\x1b]", None, &[]),
    Row::new(Both, "\x1b^", None, &[]),
    Row::new(Both, "\x1b_", None, &[]),
    Row::new(Both, "\x1b`", None, &[]),
    Row::new(Legacy, "\x1ba", Some("alt+a"), &["alt+a"]),
    Row::new(Enhanced, "\x1ba", None, &[]),
    Row::new(Legacy, "\x1bc", Some("alt+c"), &["alt+c"]),
    Row::new(Enhanced, "\x1bc", None, &[]),
    Row::new(Legacy, "\x1bd", Some("alt+d"), &["alt+d"]),
    Row::new(Enhanced, "\x1bd", None, &[]),
    Row::new(Legacy, "\x1be", Some("alt+e"), &["alt+e"]),
    Row::new(Enhanced, "\x1be", None, &[]),
    Row::new(Legacy, "\x1bg", Some("alt+g"), &["alt+g"]),
    Row::new(Enhanced, "\x1bg", None, &[]),
    Row::new(Legacy, "\x1bh", Some("alt+h"), &["alt+h"]),
    Row::new(Enhanced, "\x1bh", None, &[]),
    Row::new(Legacy, "\x1bi", Some("alt+i"), &["alt+i"]),
    Row::new(Enhanced, "\x1bi", None, &[]),
    Row::new(Legacy, "\x1bj", Some("alt+j"), &["alt+j"]),
    Row::new(Enhanced, "\x1bj", None, &[]),
    Row::new(Legacy, "\x1bk", Some("alt+k"), &["alt+k"]),
    Row::new(Enhanced, "\x1bk", None, &[]),
    Row::new(Legacy, "\x1bl", Some("alt+l"), &["alt+l"]),
    Row::new(Enhanced, "\x1bl", None, &[]),
    Row::new(Legacy, "\x1bm", Some("alt+m"), &["alt+m"]),
    Row::new(Enhanced, "\x1bm", None, &[]),
    Row::new(Legacy, "\x1bo", Some("alt+o"), &["alt+o"]),
    Row::new(Enhanced, "\x1bo", None, &[]),
    Row::new(Legacy, "\x1bq", Some("alt+q"), &["alt+q"]),
    Row::new(Enhanced, "\x1bq", None, &[]),
    Row::new(Legacy, "\x1br", Some("alt+r"), &["alt+r"]),
    Row::new(Enhanced, "\x1br", None, &[]),
    Row::new(Legacy, "\x1bs", Some("alt+s"), &["alt+s"]),
    Row::new(Enhanced, "\x1bs", None, &[]),
    Row::new(Legacy, "\x1bt", Some("alt+t"), &["alt+t"]),
    Row::new(Enhanced, "\x1bt", None, &[]),
    Row::new(Legacy, "\x1bu", Some("alt+u"), &["alt+u"]),
    Row::new(Enhanced, "\x1bu", None, &[]),
    Row::new(Legacy, "\x1bv", Some("alt+v"), &["alt+v"]),
    Row::new(Enhanced, "\x1bv", None, &[]),
    Row::new(Legacy, "\x1bw", Some("alt+w"), &["alt+w"]),
    Row::new(Enhanced, "\x1bw", None, &[]),
    Row::new(Legacy, "\x1bx", Some("alt+x"), &["alt+x"]),
    Row::new(Enhanced, "\x1bx", None, &[]),
    Row::new(Legacy, "\x1by", Some("alt+y"), &["alt+y"]),
    Row::new(Enhanced, "\x1by", None, &[]),
    Row::new(Legacy, "\x1bz", Some("alt+z"), &["alt+z"]),
    Row::new(Enhanced, "\x1bz", None, &[]),
    Row::new(Both, "\x1b{", None, &[]),
    Row::new(Both, "\x1b|", None, &[]),
    Row::new(Both, "\x1b}", None, &[]),
    Row::new(Both, "\x1b~", None, &[]),
    Row::new(Both, "\x1b\x7f", Some("alt+backspace"), &["alt+backspace"]),
];

/// Candidate identifiers tried against every row of [`MODIFIERS`].
pub const MODIFIERS_CAND: &[&str] = &[
    "escape",
    "esc",
    "enter",
    "return",
    "tab",
    "space",
    "backspace",
    "ctrl+backspace",
    "ctrl+h",
    "ctrl+space",
    "shift+enter",
    "alt+enter",
    "shift+tab",
    "up",
    "down",
    "left",
    "right",
    "home",
    "end",
    "insert",
    "delete",
    "pageUp",
    "pageDown",
    "clear",
    "f1",
    "f12",
    "a",
    "shift+a",
    "ctrl+a",
    "alt+a",
    "ctrl+alt+a",
    "ctrl+c",
    "ctrl+1",
    "super+a",
    "ctrl+super+a",
    "shift+ctrl+alt+super+a",
];

/// Rows of the `modifiers` probe.
pub const MODIFIERS: &[Row] = &[
    Row::new(Legacy, "\x1b[9;1u", Some("tab"), &["tab"]),
    Row::new(Legacy, "\x1b[13;1u", Some("enter"), &["enter", "return"]),
    Row::new(Legacy, "\x1b[27;1u", Some("escape"), &["escape", "esc"]),
    Row {
        kitty_text: Some(' '),
        text: Some(' '),
        ..Row::new(Legacy, "\x1b[32;1u", Some("space"), &["space"])
    },
    Row {
        kitty_text: Some('1'),
        text: Some('1'),
        ..Row::new(Legacy, "\x1b[49;1u", Some("1"), &["1"])
    },
    Row {
        kitty_text: Some('A'),
        text: Some('A'),
        ..Row::new(Legacy, "\x1b[65;1u", None, &[])
    },
    Row {
        kitty_text: Some('a'),
        text: Some('a'),
        ..Row::new(Legacy, "\x1b[97;1u", Some("a"), &["a"])
    },
    Row {
        kitty_text: Some('c'),
        text: Some('c'),
        ..Row::new(Legacy, "\x1b[99;1u", Some("c"), &["c"])
    },
    Row::new(Legacy, "\x1b[127;1u", Some("backspace"), &["backspace"]),
    Row::new(Legacy, "\x1b[9;2u", Some("shift+tab"), &["shift+tab"]),
    Row::new(Legacy, "\x1b[13;2u", Some("shift+enter"), &["shift+enter"]),
    Row::new(Legacy, "\x1b[27;2u", Some("shift+escape"), &[]),
    Row {
        kitty_text: Some(' '),
        text: Some(' '),
        ..Row::new(Legacy, "\x1b[32;2u", Some("shift+space"), &["shift+space"])
    },
    Row {
        kitty_text: Some('1'),
        text: Some('1'),
        ..Row::new(Legacy, "\x1b[49;2u", Some("shift+1"), &["shift+1"])
    },
    Row {
        kitty_text: Some('A'),
        text: Some('A'),
        ..Row::new(Legacy, "\x1b[65;2u", Some("shift+a"), &["shift+a"])
    },
    Row {
        kitty_text: Some('a'),
        text: Some('a'),
        ..Row::new(Legacy, "\x1b[97;2u", Some("shift+a"), &["shift+a"])
    },
    Row {
        kitty_text: Some('c'),
        text: Some('c'),
        ..Row::new(Legacy, "\x1b[99;2u", Some("shift+c"), &["shift+c"])
    },
    Row::new(
        Legacy,
        "\x1b[127;2u",
        Some("shift+backspace"),
        &["shift+backspace"],
    ),
    Row::new(Legacy, "\x1b[9;3u", Some("alt+tab"), &["alt+tab"]),
    Row::new(Legacy, "\x1b[13;3u", Some("alt+enter"), &["alt+enter"]),
    Row::new(Legacy, "\x1b[27;3u", Some("alt+escape"), &[]),
    Row::new(Legacy, "\x1b[32;3u", Some("alt+space"), &["alt+space"]),
    Row::new(Legacy, "\x1b[49;3u", Some("alt+1"), &["alt+1"]),
    Row::new(Legacy, "\x1b[65;3u", None, &[]),
    Row::new(Legacy, "\x1b[97;3u", Some("alt+a"), &["alt+a"]),
    Row::new(Legacy, "\x1b[99;3u", Some("alt+c"), &["alt+c"]),
    Row::new(
        Legacy,
        "\x1b[127;3u",
        Some("alt+backspace"),
        &["alt+backspace"],
    ),
    Row::new(
        Legacy,
        "\x1b[9;4u",
        Some("shift+alt+tab"),
        &["shift+alt+tab"],
    ),
    Row::new(
        Legacy,
        "\x1b[13;4u",
        Some("shift+alt+enter"),
        &["shift+alt+enter"],
    ),
    Row::new(Legacy, "\x1b[27;4u", Some("shift+alt+escape"), &[]),
    Row::new(
        Legacy,
        "\x1b[32;4u",
        Some("shift+alt+space"),
        &["shift+alt+space"],
    ),
    Row::new(Legacy, "\x1b[49;4u", Some("shift+alt+1"), &["shift+alt+1"]),
    Row::new(Legacy, "\x1b[65;4u", Some("shift+alt+a"), &["shift+alt+a"]),
    Row::new(Legacy, "\x1b[97;4u", Some("shift+alt+a"), &["shift+alt+a"]),
    Row::new(Legacy, "\x1b[99;4u", Some("shift+alt+c"), &["shift+alt+c"]),
    Row::new(
        Legacy,
        "\x1b[127;4u",
        Some("shift+alt+backspace"),
        &["shift+alt+backspace"],
    ),
    Row::new(Legacy, "\x1b[9;5u", Some("ctrl+tab"), &["ctrl+tab"]),
    Row::new(Legacy, "\x1b[13;5u", Some("ctrl+enter"), &["ctrl+enter"]),
    Row::new(Legacy, "\x1b[27;5u", Some("ctrl+escape"), &[]),
    Row::new(Legacy, "\x1b[32;5u", Some("ctrl+space"), &["ctrl+space"]),
    Row::new(Legacy, "\x1b[49;5u", Some("ctrl+1"), &["ctrl+1"]),
    Row::new(Legacy, "\x1b[65;5u", None, &[]),
    Row::new(Legacy, "\x1b[97;5u", Some("ctrl+a"), &["ctrl+a"]),
    Row::new(Legacy, "\x1b[99;5u", Some("ctrl+c"), &["ctrl+c"]),
    Row::new(
        Legacy,
        "\x1b[127;5u",
        Some("ctrl+backspace"),
        &["ctrl+backspace"],
    ),
    Row::new(
        Legacy,
        "\x1b[9;6u",
        Some("shift+ctrl+tab"),
        &["shift+ctrl+tab"],
    ),
    Row::new(
        Legacy,
        "\x1b[13;6u",
        Some("shift+ctrl+enter"),
        &["shift+ctrl+enter"],
    ),
    Row::new(Legacy, "\x1b[27;6u", Some("shift+ctrl+escape"), &[]),
    Row::new(
        Legacy,
        "\x1b[32;6u",
        Some("shift+ctrl+space"),
        &["shift+ctrl+space"],
    ),
    Row::new(
        Legacy,
        "\x1b[49;6u",
        Some("shift+ctrl+1"),
        &["shift+ctrl+1"],
    ),
    Row::new(
        Legacy,
        "\x1b[65;6u",
        Some("shift+ctrl+a"),
        &["shift+ctrl+a"],
    ),
    Row::new(
        Legacy,
        "\x1b[97;6u",
        Some("shift+ctrl+a"),
        &["shift+ctrl+a"],
    ),
    Row::new(
        Legacy,
        "\x1b[99;6u",
        Some("shift+ctrl+c"),
        &["shift+ctrl+c"],
    ),
    Row::new(
        Legacy,
        "\x1b[127;6u",
        Some("shift+ctrl+backspace"),
        &["shift+ctrl+backspace"],
    ),
    Row::new(Legacy, "\x1b[9;7u", Some("ctrl+alt+tab"), &["ctrl+alt+tab"]),
    Row::new(
        Legacy,
        "\x1b[13;7u",
        Some("ctrl+alt+enter"),
        &["ctrl+alt+enter"],
    ),
    Row::new(Legacy, "\x1b[27;7u", Some("ctrl+alt+escape"), &[]),
    Row::new(
        Legacy,
        "\x1b[32;7u",
        Some("ctrl+alt+space"),
        &["ctrl+alt+space"],
    ),
    Row::new(Legacy, "\x1b[49;7u", Some("ctrl+alt+1"), &["ctrl+alt+1"]),
    Row::new(Legacy, "\x1b[65;7u", None, &[]),
    Row::new(Legacy, "\x1b[97;7u", Some("ctrl+alt+a"), &["ctrl+alt+a"]),
    Row::new(Legacy, "\x1b[99;7u", Some("ctrl+alt+c"), &["ctrl+alt+c"]),
    Row::new(
        Legacy,
        "\x1b[127;7u",
        Some("ctrl+alt+backspace"),
        &["ctrl+alt+backspace"],
    ),
    Row::new(
        Legacy,
        "\x1b[9;8u",
        Some("shift+ctrl+alt+tab"),
        &["shift+ctrl+alt+tab"],
    ),
    Row::new(
        Legacy,
        "\x1b[13;8u",
        Some("shift+ctrl+alt+enter"),
        &["shift+ctrl+alt+enter"],
    ),
    Row::new(Legacy, "\x1b[27;8u", Some("shift+ctrl+alt+escape"), &[]),
    Row::new(
        Legacy,
        "\x1b[32;8u",
        Some("shift+ctrl+alt+space"),
        &["shift+ctrl+alt+space"],
    ),
    Row::new(
        Legacy,
        "\x1b[49;8u",
        Some("shift+ctrl+alt+1"),
        &["shift+ctrl+alt+1"],
    ),
    Row::new(
        Legacy,
        "\x1b[65;8u",
        Some("shift+ctrl+alt+a"),
        &["shift+ctrl+alt+a"],
    ),
    Row::new(
        Legacy,
        "\x1b[97;8u",
        Some("shift+ctrl+alt+a"),
        &["shift+ctrl+alt+a"],
    ),
    Row::new(
        Legacy,
        "\x1b[99;8u",
        Some("shift+ctrl+alt+c"),
        &["shift+ctrl+alt+c"],
    ),
    Row::new(
        Legacy,
        "\x1b[127;8u",
        Some("shift+ctrl+alt+backspace"),
        &["shift+ctrl+alt+backspace"],
    ),
    Row::new(Legacy, "\x1b[9;9u", Some("super+tab"), &["super+tab"]),
    Row::new(Legacy, "\x1b[13;9u", Some("super+enter"), &["super+enter"]),
    Row::new(Legacy, "\x1b[27;9u", Some("super+escape"), &[]),
    Row::new(Legacy, "\x1b[32;9u", Some("super+space"), &["super+space"]),
    Row::new(Legacy, "\x1b[49;9u", Some("super+1"), &["super+1"]),
    Row::new(Legacy, "\x1b[65;9u", None, &[]),
    Row::new(Legacy, "\x1b[97;9u", Some("super+a"), &["super+a"]),
    Row::new(Legacy, "\x1b[99;9u", Some("super+c"), &["super+c"]),
    Row::new(
        Legacy,
        "\x1b[127;9u",
        Some("super+backspace"),
        &["super+backspace"],
    ),
    Row::new(
        Legacy,
        "\x1b[9;10u",
        Some("shift+super+tab"),
        &["shift+super+tab"],
    ),
    Row::new(
        Legacy,
        "\x1b[13;10u",
        Some("shift+super+enter"),
        &["shift+super+enter"],
    ),
    Row::new(Legacy, "\x1b[27;10u", Some("shift+super+escape"), &[]),
    Row::new(
        Legacy,
        "\x1b[32;10u",
        Some("shift+super+space"),
        &["shift+super+space"],
    ),
    Row::new(
        Legacy,
        "\x1b[49;10u",
        Some("shift+super+1"),
        &["shift+super+1"],
    ),
    Row::new(
        Legacy,
        "\x1b[65;10u",
        Some("shift+super+a"),
        &["shift+super+a"],
    ),
    Row::new(
        Legacy,
        "\x1b[97;10u",
        Some("shift+super+a"),
        &["shift+super+a"],
    ),
    Row::new(
        Legacy,
        "\x1b[99;10u",
        Some("shift+super+c"),
        &["shift+super+c"],
    ),
    Row::new(
        Legacy,
        "\x1b[127;10u",
        Some("shift+super+backspace"),
        &["shift+super+backspace"],
    ),
    Row::new(
        Legacy,
        "\x1b[9;11u",
        Some("alt+super+tab"),
        &["alt+super+tab"],
    ),
    Row::new(
        Legacy,
        "\x1b[13;11u",
        Some("alt+super+enter"),
        &["alt+super+enter"],
    ),
    Row::new(Legacy, "\x1b[27;11u", Some("alt+super+escape"), &[]),
    Row::new(
        Legacy,
        "\x1b[32;11u",
        Some("alt+super+space"),
        &["alt+super+space"],
    ),
    Row::new(Legacy, "\x1b[49;11u", Some("alt+super+1"), &["alt+super+1"]),
    Row::new(Legacy, "\x1b[65;11u", None, &[]),
    Row::new(Legacy, "\x1b[97;11u", Some("alt+super+a"), &["alt+super+a"]),
    Row::new(Legacy, "\x1b[99;11u", Some("alt+super+c"), &["alt+super+c"]),
    Row::new(
        Legacy,
        "\x1b[127;11u",
        Some("alt+super+backspace"),
        &["alt+super+backspace"],
    ),
    Row::new(
        Legacy,
        "\x1b[9;12u",
        Some("shift+alt+super+tab"),
        &["shift+alt+super+tab"],
    ),
    Row::new(
        Legacy,
        "\x1b[13;12u",
        Some("shift+alt+super+enter"),
        &["shift+alt+super+enter"],
    ),
    Row::new(Legacy, "\x1b[27;12u", Some("shift+alt+super+escape"), &[]),
    Row::new(
        Legacy,
        "\x1b[32;12u",
        Some("shift+alt+super+space"),
        &["shift+alt+super+space"],
    ),
    Row::new(
        Legacy,
        "\x1b[49;12u",
        Some("shift+alt+super+1"),
        &["shift+alt+super+1"],
    ),
    Row::new(
        Legacy,
        "\x1b[65;12u",
        Some("shift+alt+super+a"),
        &["shift+alt+super+a"],
    ),
    Row::new(
        Legacy,
        "\x1b[97;12u",
        Some("shift+alt+super+a"),
        &["shift+alt+super+a"],
    ),
    Row::new(
        Legacy,
        "\x1b[99;12u",
        Some("shift+alt+super+c"),
        &["shift+alt+super+c"],
    ),
    Row::new(
        Legacy,
        "\x1b[127;12u",
        Some("shift+alt+super+backspace"),
        &["shift+alt+super+backspace"],
    ),
    Row::new(
        Legacy,
        "\x1b[9;13u",
        Some("ctrl+super+tab"),
        &["ctrl+super+tab"],
    ),
    Row::new(
        Legacy,
        "\x1b[13;13u",
        Some("ctrl+super+enter"),
        &["ctrl+super+enter"],
    ),
    Row::new(Legacy, "\x1b[27;13u", Some("ctrl+super+escape"), &[]),
    Row::new(
        Legacy,
        "\x1b[32;13u",
        Some("ctrl+super+space"),
        &["ctrl+super+space"],
    ),
    Row::new(
        Legacy,
        "\x1b[49;13u",
        Some("ctrl+super+1"),
        &["ctrl+super+1"],
    ),
    Row::new(Legacy, "\x1b[65;13u", None, &[]),
    Row::new(
        Legacy,
        "\x1b[97;13u",
        Some("ctrl+super+a"),
        &["ctrl+super+a"],
    ),
    Row::new(
        Legacy,
        "\x1b[99;13u",
        Some("ctrl+super+c"),
        &["ctrl+super+c"],
    ),
    Row::new(
        Legacy,
        "\x1b[127;13u",
        Some("ctrl+super+backspace"),
        &["ctrl+super+backspace"],
    ),
    Row::new(
        Legacy,
        "\x1b[9;14u",
        Some("shift+ctrl+super+tab"),
        &["shift+ctrl+super+tab"],
    ),
    Row::new(
        Legacy,
        "\x1b[13;14u",
        Some("shift+ctrl+super+enter"),
        &["shift+ctrl+super+enter"],
    ),
    Row::new(Legacy, "\x1b[27;14u", Some("shift+ctrl+super+escape"), &[]),
    Row::new(
        Legacy,
        "\x1b[32;14u",
        Some("shift+ctrl+super+space"),
        &["shift+ctrl+super+space"],
    ),
    Row::new(
        Legacy,
        "\x1b[49;14u",
        Some("shift+ctrl+super+1"),
        &["shift+ctrl+super+1"],
    ),
    Row::new(
        Legacy,
        "\x1b[65;14u",
        Some("shift+ctrl+super+a"),
        &["shift+ctrl+super+a"],
    ),
    Row::new(
        Legacy,
        "\x1b[97;14u",
        Some("shift+ctrl+super+a"),
        &["shift+ctrl+super+a"],
    ),
    Row::new(
        Legacy,
        "\x1b[99;14u",
        Some("shift+ctrl+super+c"),
        &["shift+ctrl+super+c"],
    ),
    Row::new(
        Legacy,
        "\x1b[127;14u",
        Some("shift+ctrl+super+backspace"),
        &["shift+ctrl+super+backspace"],
    ),
    Row::new(
        Legacy,
        "\x1b[9;15u",
        Some("ctrl+alt+super+tab"),
        &["ctrl+alt+super+tab"],
    ),
    Row::new(
        Legacy,
        "\x1b[13;15u",
        Some("ctrl+alt+super+enter"),
        &["ctrl+alt+super+enter"],
    ),
    Row::new(Legacy, "\x1b[27;15u", Some("ctrl+alt+super+escape"), &[]),
    Row::new(
        Legacy,
        "\x1b[32;15u",
        Some("ctrl+alt+super+space"),
        &["ctrl+alt+super+space"],
    ),
    Row::new(
        Legacy,
        "\x1b[49;15u",
        Some("ctrl+alt+super+1"),
        &["ctrl+alt+super+1"],
    ),
    Row::new(Legacy, "\x1b[65;15u", None, &[]),
    Row::new(
        Legacy,
        "\x1b[97;15u",
        Some("ctrl+alt+super+a"),
        &["ctrl+alt+super+a"],
    ),
    Row::new(
        Legacy,
        "\x1b[99;15u",
        Some("ctrl+alt+super+c"),
        &["ctrl+alt+super+c"],
    ),
    Row::new(
        Legacy,
        "\x1b[127;15u",
        Some("ctrl+alt+super+backspace"),
        &["ctrl+alt+super+backspace"],
    ),
    Row::new(
        Legacy,
        "\x1b[9;16u",
        Some("shift+ctrl+alt+super+tab"),
        &["shift+ctrl+alt+super+tab"],
    ),
    Row::new(
        Legacy,
        "\x1b[13;16u",
        Some("shift+ctrl+alt+super+enter"),
        &["shift+ctrl+alt+super+enter"],
    ),
    Row::new(
        Legacy,
        "\x1b[27;16u",
        Some("shift+ctrl+alt+super+escape"),
        &[],
    ),
    Row::new(
        Legacy,
        "\x1b[32;16u",
        Some("shift+ctrl+alt+super+space"),
        &["shift+ctrl+alt+super+space"],
    ),
    Row::new(
        Legacy,
        "\x1b[49;16u",
        Some("shift+ctrl+alt+super+1"),
        &["shift+ctrl+alt+super+1"],
    ),
    Row::new(
        Legacy,
        "\x1b[65;16u",
        Some("shift+ctrl+alt+super+a"),
        &["shift+ctrl+alt+super+a"],
    ),
    Row::new(
        Legacy,
        "\x1b[97;16u",
        Some("shift+ctrl+alt+super+a"),
        &["shift+ctrl+alt+super+a"],
    ),
    Row::new(
        Legacy,
        "\x1b[99;16u",
        Some("shift+ctrl+alt+super+c"),
        &["shift+ctrl+alt+super+c"],
    ),
    Row::new(
        Legacy,
        "\x1b[127;16u",
        Some("shift+ctrl+alt+super+backspace"),
        &["shift+ctrl+alt+super+backspace"],
    ),
    Row::new(Legacy, "\x1b[9;17u", None, &[]),
    Row::new(Legacy, "\x1b[13;17u", None, &[]),
    Row::new(Legacy, "\x1b[27;17u", None, &[]),
    Row::new(Legacy, "\x1b[32;17u", None, &[]),
    Row::new(Legacy, "\x1b[49;17u", None, &[]),
    Row::new(Legacy, "\x1b[65;17u", None, &[]),
    Row::new(Legacy, "\x1b[97;17u", None, &[]),
    Row::new(Legacy, "\x1b[99;17u", None, &[]),
    Row::new(Legacy, "\x1b[127;17u", None, &[]),
    Row::new(Legacy, "\x1b[9;33u", None, &[]),
    Row::new(Legacy, "\x1b[13;33u", None, &[]),
    Row::new(Legacy, "\x1b[27;33u", None, &[]),
    Row::new(Legacy, "\x1b[32;33u", None, &[]),
    Row::new(Legacy, "\x1b[49;33u", None, &[]),
    Row::new(Legacy, "\x1b[65;33u", None, &[]),
    Row::new(Legacy, "\x1b[97;33u", None, &[]),
    Row::new(Legacy, "\x1b[99;33u", None, &[]),
    Row::new(Legacy, "\x1b[127;33u", None, &[]),
    Row::new(Legacy, "\x1b[9;257u", None, &[]),
    Row::new(Legacy, "\x1b[13;257u", None, &[]),
    Row::new(Legacy, "\x1b[27;257u", None, &[]),
    Row::new(Legacy, "\x1b[32;257u", None, &[]),
    Row::new(Legacy, "\x1b[49;257u", None, &[]),
    Row::new(Legacy, "\x1b[65;257u", None, &[]),
    Row::new(Legacy, "\x1b[97;257u", None, &[]),
    Row::new(Legacy, "\x1b[99;257u", None, &[]),
    Row::new(Legacy, "\x1b[127;257u", None, &[]),
];

/// Candidate identifiers tried against every row of [`KEYPAD`].
pub const KEYPAD_CAND: &[&str] = &[
    "escape",
    "esc",
    "enter",
    "return",
    "tab",
    "space",
    "backspace",
    "ctrl+backspace",
    "ctrl+h",
    "ctrl+space",
    "shift+enter",
    "alt+enter",
    "shift+tab",
    "up",
    "down",
    "left",
    "right",
    "home",
    "end",
    "insert",
    "delete",
    "pageUp",
    "pageDown",
    "clear",
    "f1",
    "f12",
    "a",
    "shift+a",
    "ctrl+a",
    "alt+a",
    "ctrl+alt+a",
    "0",
    "1",
    "9",
    "/",
    "*",
    "-",
    ".",
    ",",
    "=",
    "+",
    "shift++",
    "ctrl++",
];

/// Rows of the `keypad` probe.
pub const KEYPAD: &[Row] = &[
    Row::new(Legacy, "\x1b[57398;1u", None, &[]),
    Row::new(Legacy, "\x1b[57398;2u", None, &[]),
    Row::new(Legacy, "\x1b[57398;5u", None, &[]),
    Row::new(Legacy, "\x1b[57398;65u", None, &[]),
    Row::new(Legacy, "\x1b[57398;193u", None, &[]),
    Row {
        kitty_text: Some('0'),
        text: Some('0'),
        ..Row::new(Legacy, "\x1b[57399;1u", Some("0"), &["0"])
    },
    Row {
        kitty_text: Some('0'),
        text: Some('0'),
        ..Row::new(Legacy, "\x1b[57399;2u", Some("shift+0"), &["shift+0"])
    },
    Row::new(Legacy, "\x1b[57399;5u", Some("ctrl+0"), &["ctrl+0"]),
    Row {
        kitty_text: Some('0'),
        text: Some('0'),
        ..Row::new(Legacy, "\x1b[57399;65u", Some("0"), &["0"])
    },
    Row {
        kitty_text: Some('0'),
        text: Some('0'),
        ..Row::new(Legacy, "\x1b[57399;193u", Some("0"), &["0"])
    },
    Row {
        kitty_text: Some('1'),
        text: Some('1'),
        ..Row::new(Legacy, "\x1b[57400;1u", Some("1"), &["1"])
    },
    Row {
        kitty_text: Some('1'),
        text: Some('1'),
        ..Row::new(Legacy, "\x1b[57400;2u", Some("shift+1"), &["shift+1"])
    },
    Row::new(Legacy, "\x1b[57400;5u", Some("ctrl+1"), &["ctrl+1"]),
    Row {
        kitty_text: Some('1'),
        text: Some('1'),
        ..Row::new(Legacy, "\x1b[57400;65u", Some("1"), &["1"])
    },
    Row {
        kitty_text: Some('1'),
        text: Some('1'),
        ..Row::new(Legacy, "\x1b[57400;193u", Some("1"), &["1"])
    },
    Row {
        kitty_text: Some('2'),
        text: Some('2'),
        ..Row::new(Legacy, "\x1b[57401;1u", Some("2"), &["2"])
    },
    Row {
        kitty_text: Some('2'),
        text: Some('2'),
        ..Row::new(Legacy, "\x1b[57401;2u", Some("shift+2"), &["shift+2"])
    },
    Row::new(Legacy, "\x1b[57401;5u", Some("ctrl+2"), &["ctrl+2"]),
    Row {
        kitty_text: Some('2'),
        text: Some('2'),
        ..Row::new(Legacy, "\x1b[57401;65u", Some("2"), &["2"])
    },
    Row {
        kitty_text: Some('2'),
        text: Some('2'),
        ..Row::new(Legacy, "\x1b[57401;193u", Some("2"), &["2"])
    },
    Row {
        kitty_text: Some('3'),
        text: Some('3'),
        ..Row::new(Legacy, "\x1b[57402;1u", Some("3"), &["3"])
    },
    Row {
        kitty_text: Some('3'),
        text: Some('3'),
        ..Row::new(Legacy, "\x1b[57402;2u", Some("shift+3"), &["shift+3"])
    },
    Row::new(Legacy, "\x1b[57402;5u", Some("ctrl+3"), &["ctrl+3"]),
    Row {
        kitty_text: Some('3'),
        text: Some('3'),
        ..Row::new(Legacy, "\x1b[57402;65u", Some("3"), &["3"])
    },
    Row {
        kitty_text: Some('3'),
        text: Some('3'),
        ..Row::new(Legacy, "\x1b[57402;193u", Some("3"), &["3"])
    },
    Row {
        kitty_text: Some('4'),
        text: Some('4'),
        ..Row::new(Legacy, "\x1b[57403;1u", Some("4"), &["4"])
    },
    Row {
        kitty_text: Some('4'),
        text: Some('4'),
        ..Row::new(Legacy, "\x1b[57403;2u", Some("shift+4"), &["shift+4"])
    },
    Row::new(Legacy, "\x1b[57403;5u", Some("ctrl+4"), &["ctrl+4"]),
    Row {
        kitty_text: Some('4'),
        text: Some('4'),
        ..Row::new(Legacy, "\x1b[57403;65u", Some("4"), &["4"])
    },
    Row {
        kitty_text: Some('4'),
        text: Some('4'),
        ..Row::new(Legacy, "\x1b[57403;193u", Some("4"), &["4"])
    },
    Row {
        kitty_text: Some('5'),
        text: Some('5'),
        ..Row::new(Legacy, "\x1b[57404;1u", Some("5"), &["5"])
    },
    Row {
        kitty_text: Some('5'),
        text: Some('5'),
        ..Row::new(Legacy, "\x1b[57404;2u", Some("shift+5"), &["shift+5"])
    },
    Row::new(Legacy, "\x1b[57404;5u", Some("ctrl+5"), &["ctrl+5"]),
    Row {
        kitty_text: Some('5'),
        text: Some('5'),
        ..Row::new(Legacy, "\x1b[57404;65u", Some("5"), &["5"])
    },
    Row {
        kitty_text: Some('5'),
        text: Some('5'),
        ..Row::new(Legacy, "\x1b[57404;193u", Some("5"), &["5"])
    },
    Row {
        kitty_text: Some('6'),
        text: Some('6'),
        ..Row::new(Legacy, "\x1b[57405;1u", Some("6"), &["6"])
    },
    Row {
        kitty_text: Some('6'),
        text: Some('6'),
        ..Row::new(Legacy, "\x1b[57405;2u", Some("shift+6"), &["shift+6"])
    },
    Row::new(Legacy, "\x1b[57405;5u", Some("ctrl+6"), &["ctrl+6"]),
    Row {
        kitty_text: Some('6'),
        text: Some('6'),
        ..Row::new(Legacy, "\x1b[57405;65u", Some("6"), &["6"])
    },
    Row {
        kitty_text: Some('6'),
        text: Some('6'),
        ..Row::new(Legacy, "\x1b[57405;193u", Some("6"), &["6"])
    },
    Row {
        kitty_text: Some('7'),
        text: Some('7'),
        ..Row::new(Legacy, "\x1b[57406;1u", Some("7"), &["7"])
    },
    Row {
        kitty_text: Some('7'),
        text: Some('7'),
        ..Row::new(Legacy, "\x1b[57406;2u", Some("shift+7"), &["shift+7"])
    },
    Row::new(Legacy, "\x1b[57406;5u", Some("ctrl+7"), &["ctrl+7"]),
    Row {
        kitty_text: Some('7'),
        text: Some('7'),
        ..Row::new(Legacy, "\x1b[57406;65u", Some("7"), &["7"])
    },
    Row {
        kitty_text: Some('7'),
        text: Some('7'),
        ..Row::new(Legacy, "\x1b[57406;193u", Some("7"), &["7"])
    },
    Row {
        kitty_text: Some('8'),
        text: Some('8'),
        ..Row::new(Legacy, "\x1b[57407;1u", Some("8"), &["8"])
    },
    Row {
        kitty_text: Some('8'),
        text: Some('8'),
        ..Row::new(Legacy, "\x1b[57407;2u", Some("shift+8"), &["shift+8"])
    },
    Row::new(Legacy, "\x1b[57407;5u", Some("ctrl+8"), &["ctrl+8"]),
    Row {
        kitty_text: Some('8'),
        text: Some('8'),
        ..Row::new(Legacy, "\x1b[57407;65u", Some("8"), &["8"])
    },
    Row {
        kitty_text: Some('8'),
        text: Some('8'),
        ..Row::new(Legacy, "\x1b[57407;193u", Some("8"), &["8"])
    },
    Row {
        kitty_text: Some('9'),
        text: Some('9'),
        ..Row::new(Legacy, "\x1b[57408;1u", Some("9"), &["9"])
    },
    Row {
        kitty_text: Some('9'),
        text: Some('9'),
        ..Row::new(Legacy, "\x1b[57408;2u", Some("shift+9"), &["shift+9"])
    },
    Row::new(Legacy, "\x1b[57408;5u", Some("ctrl+9"), &["ctrl+9"]),
    Row {
        kitty_text: Some('9'),
        text: Some('9'),
        ..Row::new(Legacy, "\x1b[57408;65u", Some("9"), &["9"])
    },
    Row {
        kitty_text: Some('9'),
        text: Some('9'),
        ..Row::new(Legacy, "\x1b[57408;193u", Some("9"), &["9"])
    },
    Row {
        kitty_text: Some('.'),
        text: Some('.'),
        ..Row::new(Legacy, "\x1b[57409;1u", Some("."), &["."])
    },
    Row {
        kitty_text: Some('.'),
        text: Some('.'),
        ..Row::new(Legacy, "\x1b[57409;2u", Some("shift+."), &["shift+."])
    },
    Row::new(Legacy, "\x1b[57409;5u", Some("ctrl+."), &["ctrl+."]),
    Row {
        kitty_text: Some('.'),
        text: Some('.'),
        ..Row::new(Legacy, "\x1b[57409;65u", Some("."), &["."])
    },
    Row {
        kitty_text: Some('.'),
        text: Some('.'),
        ..Row::new(Legacy, "\x1b[57409;193u", Some("."), &["."])
    },
    Row {
        kitty_text: Some('/'),
        text: Some('/'),
        ..Row::new(Legacy, "\x1b[57410;1u", Some("/"), &["/"])
    },
    Row {
        kitty_text: Some('/'),
        text: Some('/'),
        ..Row::new(Legacy, "\x1b[57410;2u", Some("shift+/"), &["shift+/"])
    },
    Row::new(Legacy, "\x1b[57410;5u", Some("ctrl+/"), &["ctrl+/"]),
    Row {
        kitty_text: Some('/'),
        text: Some('/'),
        ..Row::new(Legacy, "\x1b[57410;65u", Some("/"), &["/"])
    },
    Row {
        kitty_text: Some('/'),
        text: Some('/'),
        ..Row::new(Legacy, "\x1b[57410;193u", Some("/"), &["/"])
    },
    Row {
        kitty_text: Some('*'),
        text: Some('*'),
        ..Row::new(Legacy, "\x1b[57411;1u", Some("*"), &["*"])
    },
    Row {
        kitty_text: Some('*'),
        text: Some('*'),
        ..Row::new(Legacy, "\x1b[57411;2u", Some("shift+*"), &["shift+*"])
    },
    Row::new(Legacy, "\x1b[57411;5u", Some("ctrl+*"), &["ctrl+*"]),
    Row {
        kitty_text: Some('*'),
        text: Some('*'),
        ..Row::new(Legacy, "\x1b[57411;65u", Some("*"), &["*"])
    },
    Row {
        kitty_text: Some('*'),
        text: Some('*'),
        ..Row::new(Legacy, "\x1b[57411;193u", Some("*"), &["*"])
    },
    Row {
        kitty_text: Some('-'),
        text: Some('-'),
        ..Row::new(Legacy, "\x1b[57412;1u", Some("-"), &["-"])
    },
    Row {
        kitty_text: Some('-'),
        text: Some('-'),
        ..Row::new(Legacy, "\x1b[57412;2u", Some("shift+-"), &["shift+-"])
    },
    Row::new(Legacy, "\x1b[57412;5u", Some("ctrl+-"), &["ctrl+-"]),
    Row {
        kitty_text: Some('-'),
        text: Some('-'),
        ..Row::new(Legacy, "\x1b[57412;65u", Some("-"), &["-"])
    },
    Row {
        kitty_text: Some('-'),
        text: Some('-'),
        ..Row::new(Legacy, "\x1b[57412;193u", Some("-"), &["-"])
    },
    Row {
        kitty_text: Some('+'),
        text: Some('+'),
        ..Row::new(Legacy, "\x1b[57413;1u", Some("+"), &["+"])
    },
    Row {
        kitty_text: Some('+'),
        text: Some('+'),
        ..Row::new(Legacy, "\x1b[57413;2u", Some("shift++"), &["shift++"])
    },
    Row::new(Legacy, "\x1b[57413;5u", Some("ctrl++"), &["ctrl++"]),
    Row {
        kitty_text: Some('+'),
        text: Some('+'),
        ..Row::new(Legacy, "\x1b[57413;65u", Some("+"), &["+"])
    },
    Row {
        kitty_text: Some('+'),
        text: Some('+'),
        ..Row::new(Legacy, "\x1b[57413;193u", Some("+"), &["+"])
    },
    Row::new(Legacy, "\x1b[57414;1u", Some("enter"), &["enter", "return"]),
    Row::new(
        Legacy,
        "\x1b[57414;2u",
        Some("shift+enter"),
        &["shift+enter"],
    ),
    Row::new(Legacy, "\x1b[57414;5u", Some("ctrl+enter"), &["ctrl+enter"]),
    Row::new(
        Legacy,
        "\x1b[57414;65u",
        Some("enter"),
        &["enter", "return"],
    ),
    Row::new(
        Legacy,
        "\x1b[57414;193u",
        Some("enter"),
        &["enter", "return"],
    ),
    Row {
        kitty_text: Some('='),
        text: Some('='),
        ..Row::new(Legacy, "\x1b[57415;1u", Some("="), &["="])
    },
    Row {
        kitty_text: Some('='),
        text: Some('='),
        ..Row::new(Legacy, "\x1b[57415;2u", Some("shift+="), &["shift+="])
    },
    Row::new(Legacy, "\x1b[57415;5u", Some("ctrl+="), &["ctrl+="]),
    Row {
        kitty_text: Some('='),
        text: Some('='),
        ..Row::new(Legacy, "\x1b[57415;65u", Some("="), &["="])
    },
    Row {
        kitty_text: Some('='),
        text: Some('='),
        ..Row::new(Legacy, "\x1b[57415;193u", Some("="), &["="])
    },
    Row {
        kitty_text: Some(','),
        text: Some(','),
        ..Row::new(Legacy, "\x1b[57416;1u", Some(","), &[","])
    },
    Row {
        kitty_text: Some(','),
        text: Some(','),
        ..Row::new(Legacy, "\x1b[57416;2u", Some("shift+,"), &["shift+,"])
    },
    Row::new(Legacy, "\x1b[57416;5u", Some("ctrl+,"), &["ctrl+,"]),
    Row {
        kitty_text: Some(','),
        text: Some(','),
        ..Row::new(Legacy, "\x1b[57416;65u", Some(","), &[","])
    },
    Row {
        kitty_text: Some(','),
        text: Some(','),
        ..Row::new(Legacy, "\x1b[57416;193u", Some(","), &[","])
    },
    Row::new(Legacy, "\x1b[57417;1u", Some("left"), &["left"]),
    Row::new(Legacy, "\x1b[57417;2u", Some("shift+left"), &["shift+left"]),
    Row::new(Legacy, "\x1b[57417;5u", Some("ctrl+left"), &["ctrl+left"]),
    Row::new(Legacy, "\x1b[57417;65u", Some("left"), &["left"]),
    Row::new(Legacy, "\x1b[57417;193u", Some("left"), &["left"]),
    Row::new(Legacy, "\x1b[57418;1u", Some("right"), &["right"]),
    Row::new(
        Legacy,
        "\x1b[57418;2u",
        Some("shift+right"),
        &["shift+right"],
    ),
    Row::new(Legacy, "\x1b[57418;5u", Some("ctrl+right"), &["ctrl+right"]),
    Row::new(Legacy, "\x1b[57418;65u", Some("right"), &["right"]),
    Row::new(Legacy, "\x1b[57418;193u", Some("right"), &["right"]),
    Row::new(Legacy, "\x1b[57419;1u", Some("up"), &["up"]),
    Row::new(Legacy, "\x1b[57419;2u", Some("shift+up"), &["shift+up"]),
    Row::new(Legacy, "\x1b[57419;5u", Some("ctrl+up"), &["ctrl+up"]),
    Row::new(Legacy, "\x1b[57419;65u", Some("up"), &["up"]),
    Row::new(Legacy, "\x1b[57419;193u", Some("up"), &["up"]),
    Row::new(Legacy, "\x1b[57420;1u", Some("down"), &["down"]),
    Row::new(Legacy, "\x1b[57420;2u", Some("shift+down"), &["shift+down"]),
    Row::new(Legacy, "\x1b[57420;5u", Some("ctrl+down"), &["ctrl+down"]),
    Row::new(Legacy, "\x1b[57420;65u", Some("down"), &["down"]),
    Row::new(Legacy, "\x1b[57420;193u", Some("down"), &["down"]),
    Row::new(Legacy, "\x1b[57421;1u", Some("pageUp"), &["pageUp"]),
    Row::new(
        Legacy,
        "\x1b[57421;2u",
        Some("shift+pageUp"),
        &["shift+pageUp"],
    ),
    Row::new(
        Legacy,
        "\x1b[57421;5u",
        Some("ctrl+pageUp"),
        &["ctrl+pageUp"],
    ),
    Row::new(Legacy, "\x1b[57421;65u", Some("pageUp"), &["pageUp"]),
    Row::new(Legacy, "\x1b[57421;193u", Some("pageUp"), &["pageUp"]),
    Row::new(Legacy, "\x1b[57422;1u", Some("pageDown"), &["pageDown"]),
    Row::new(
        Legacy,
        "\x1b[57422;2u",
        Some("shift+pageDown"),
        &["shift+pageDown"],
    ),
    Row::new(
        Legacy,
        "\x1b[57422;5u",
        Some("ctrl+pageDown"),
        &["ctrl+pageDown"],
    ),
    Row::new(Legacy, "\x1b[57422;65u", Some("pageDown"), &["pageDown"]),
    Row::new(Legacy, "\x1b[57422;193u", Some("pageDown"), &["pageDown"]),
    Row::new(Legacy, "\x1b[57423;1u", Some("home"), &["home"]),
    Row::new(Legacy, "\x1b[57423;2u", Some("shift+home"), &["shift+home"]),
    Row::new(Legacy, "\x1b[57423;5u", Some("ctrl+home"), &["ctrl+home"]),
    Row::new(Legacy, "\x1b[57423;65u", Some("home"), &["home"]),
    Row::new(Legacy, "\x1b[57423;193u", Some("home"), &["home"]),
    Row::new(Legacy, "\x1b[57424;1u", Some("end"), &["end"]),
    Row::new(Legacy, "\x1b[57424;2u", Some("shift+end"), &["shift+end"]),
    Row::new(Legacy, "\x1b[57424;5u", Some("ctrl+end"), &["ctrl+end"]),
    Row::new(Legacy, "\x1b[57424;65u", Some("end"), &["end"]),
    Row::new(Legacy, "\x1b[57424;193u", Some("end"), &["end"]),
    Row::new(Legacy, "\x1b[57425;1u", Some("insert"), &["insert"]),
    Row::new(
        Legacy,
        "\x1b[57425;2u",
        Some("shift+insert"),
        &["shift+insert"],
    ),
    Row::new(
        Legacy,
        "\x1b[57425;5u",
        Some("ctrl+insert"),
        &["ctrl+insert"],
    ),
    Row::new(Legacy, "\x1b[57425;65u", Some("insert"), &["insert"]),
    Row::new(Legacy, "\x1b[57425;193u", Some("insert"), &["insert"]),
    Row::new(Legacy, "\x1b[57426;1u", Some("delete"), &["delete"]),
    Row::new(
        Legacy,
        "\x1b[57426;2u",
        Some("shift+delete"),
        &["shift+delete"],
    ),
    Row::new(
        Legacy,
        "\x1b[57426;5u",
        Some("ctrl+delete"),
        &["ctrl+delete"],
    ),
    Row::new(Legacy, "\x1b[57426;65u", Some("delete"), &["delete"]),
    Row::new(Legacy, "\x1b[57426;193u", Some("delete"), &["delete"]),
    Row::new(Legacy, "\x1b[57427;1u", None, &[]),
    Row::new(Legacy, "\x1b[57427;2u", None, &[]),
    Row::new(Legacy, "\x1b[57427;5u", None, &[]),
    Row::new(Legacy, "\x1b[57427;65u", None, &[]),
    Row::new(Legacy, "\x1b[57427;193u", None, &[]),
];

/// Candidate identifiers tried against every row of [`LAYOUT`].
pub const LAYOUT_CAND: &[&str] = &[
    "c",
    "ctrl+c",
    "ctrl+v",
    "ctrl+/",
    "shift+a",
    "shift+c",
    "ctrl+shift+c",
    "ctrl+shift+p",
];

/// Rows of the `layout` probe.
pub const LAYOUT: &[Row] = &[
    Row {
        kitty_text: Some('A'),
        text: Some('A'),
        ..Row::new(Legacy, "\x1b[65u", None, &[])
    },
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[65;6:2u",
            Some("shift+ctrl+a"),
            &["shift+ctrl+a"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[65;5:3u", None, &[])
    },
    Row {
        kitty_text: Some('A'),
        text: Some('A'),
        ..Row::new(Legacy, "\x1b[65:u", None, &[])
    },
    Row {
        kitty_text: Some('A'),
        text: Some('A'),
        ..Row::new(Legacy, "\x1b[65:;1u", None, &[])
    },
    Row {
        kitty_text: Some('A'),
        text: Some('A'),
        ..Row::new(Legacy, "\x1b[65:;2u", Some("shift+a"), &["shift+a"])
    },
    Row::new(Legacy, "\x1b[65:;5u", None, &[]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[65:;6:2u",
            Some("shift+ctrl+a"),
            &["shift+ctrl+a"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[65:;5:3u", None, &[])
    },
    Row {
        kitty_text: Some('A'),
        text: Some('A'),
        ..Row::new(Legacy, "\x1b[65:67u", None, &[])
    },
    Row {
        kitty_text: Some('A'),
        text: Some('A'),
        ..Row::new(Legacy, "\x1b[65:67;1u", None, &[])
    },
    Row {
        kitty_text: Some('C'),
        text: Some('C'),
        ..Row::new(Legacy, "\x1b[65:67;2u", Some("shift+a"), &["shift+a"])
    },
    Row::new(Legacy, "\x1b[65:67;5u", None, &[]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[65:67;6:2u",
            Some("shift+ctrl+a"),
            &["shift+ctrl+a"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[65:67;5:3u", None, &[])
    },
    Row {
        kitty_text: Some('A'),
        text: Some('A'),
        ..Row::new(Legacy, "\x1b[65::99u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('A'),
        text: Some('A'),
        ..Row::new(Legacy, "\x1b[65::99;1u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('A'),
        text: Some('A'),
        ..Row::new(Legacy, "\x1b[65::99;2u", Some("shift+a"), &["shift+a"])
    },
    Row::new(Legacy, "\x1b[65::99;5u", Some("ctrl+c"), &["ctrl+c"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[65::99;6:2u",
            Some("shift+ctrl+a"),
            &["shift+ctrl+a"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[65::99;5:3u", Some("ctrl+c"), &["ctrl+c"])
    },
    Row {
        kitty_text: Some('A'),
        text: Some('A'),
        ..Row::new(Legacy, "\x1b[65:67:99u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('A'),
        text: Some('A'),
        ..Row::new(Legacy, "\x1b[65:67:99;1u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('C'),
        text: Some('C'),
        ..Row::new(Legacy, "\x1b[65:67:99;2u", Some("shift+a"), &["shift+a"])
    },
    Row::new(Legacy, "\x1b[65:67:99;5u", Some("ctrl+c"), &["ctrl+c"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[65:67:99;6:2u",
            Some("shift+ctrl+a"),
            &["shift+ctrl+a"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[65:67:99;5:3u", Some("ctrl+c"), &["ctrl+c"])
    },
    Row {
        kitty_text: Some('A'),
        text: Some('A'),
        ..Row::new(Legacy, "\x1b[65::118u", Some("v"), &["v"])
    },
    Row {
        kitty_text: Some('A'),
        text: Some('A'),
        ..Row::new(Legacy, "\x1b[65::118;1u", Some("v"), &["v"])
    },
    Row {
        kitty_text: Some('A'),
        text: Some('A'),
        ..Row::new(Legacy, "\x1b[65::118;2u", Some("shift+a"), &["shift+a"])
    },
    Row::new(Legacy, "\x1b[65::118;5u", Some("ctrl+v"), &["ctrl+v"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[65::118;6:2u",
            Some("shift+ctrl+a"),
            &["shift+ctrl+a"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[65::118;5:3u", Some("ctrl+v"), &["ctrl+v"])
    },
    Row {
        kitty_text: Some('a'),
        text: Some('a'),
        ..Row::new(Legacy, "\x1b[97u", Some("a"), &["a"])
    },
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[97;6:2u",
            Some("shift+ctrl+a"),
            &["shift+ctrl+a"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[97;5:3u", Some("ctrl+a"), &["ctrl+a"])
    },
    Row {
        kitty_text: Some('a'),
        text: Some('a'),
        ..Row::new(Legacy, "\x1b[97:u", Some("a"), &["a"])
    },
    Row {
        kitty_text: Some('a'),
        text: Some('a'),
        ..Row::new(Legacy, "\x1b[97:;1u", Some("a"), &["a"])
    },
    Row {
        kitty_text: Some('a'),
        text: Some('a'),
        ..Row::new(Legacy, "\x1b[97:;2u", Some("shift+a"), &["shift+a"])
    },
    Row::new(Legacy, "\x1b[97:;5u", Some("ctrl+a"), &["ctrl+a"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[97:;6:2u",
            Some("shift+ctrl+a"),
            &["shift+ctrl+a"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[97:;5:3u", Some("ctrl+a"), &["ctrl+a"])
    },
    Row {
        kitty_text: Some('a'),
        text: Some('a'),
        ..Row::new(Legacy, "\x1b[97:67u", Some("a"), &["a"])
    },
    Row {
        kitty_text: Some('a'),
        text: Some('a'),
        ..Row::new(Legacy, "\x1b[97:67;1u", Some("a"), &["a"])
    },
    Row {
        kitty_text: Some('C'),
        text: Some('C'),
        ..Row::new(Legacy, "\x1b[97:67;2u", Some("shift+a"), &["shift+a"])
    },
    Row::new(Legacy, "\x1b[97:67;5u", Some("ctrl+a"), &["ctrl+a"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[97:67;6:2u",
            Some("shift+ctrl+a"),
            &["shift+ctrl+a"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[97:67;5:3u", Some("ctrl+a"), &["ctrl+a"])
    },
    Row {
        kitty_text: Some('a'),
        text: Some('a'),
        ..Row::new(Legacy, "\x1b[97::99u", Some("a"), &["a"])
    },
    Row {
        kitty_text: Some('a'),
        text: Some('a'),
        ..Row::new(Legacy, "\x1b[97::99;1u", Some("a"), &["a"])
    },
    Row {
        kitty_text: Some('a'),
        text: Some('a'),
        ..Row::new(Legacy, "\x1b[97::99;2u", Some("shift+a"), &["shift+a"])
    },
    Row::new(Legacy, "\x1b[97::99;5u", Some("ctrl+a"), &["ctrl+a"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[97::99;6:2u",
            Some("shift+ctrl+a"),
            &["shift+ctrl+a"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[97::99;5:3u", Some("ctrl+a"), &["ctrl+a"])
    },
    Row {
        kitty_text: Some('a'),
        text: Some('a'),
        ..Row::new(Legacy, "\x1b[97:67:99u", Some("a"), &["a"])
    },
    Row {
        kitty_text: Some('a'),
        text: Some('a'),
        ..Row::new(Legacy, "\x1b[97:67:99;1u", Some("a"), &["a"])
    },
    Row {
        kitty_text: Some('C'),
        text: Some('C'),
        ..Row::new(Legacy, "\x1b[97:67:99;2u", Some("shift+a"), &["shift+a"])
    },
    Row::new(Legacy, "\x1b[97:67:99;5u", Some("ctrl+a"), &["ctrl+a"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[97:67:99;6:2u",
            Some("shift+ctrl+a"),
            &["shift+ctrl+a"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[97:67:99;5:3u", Some("ctrl+a"), &["ctrl+a"])
    },
    Row {
        kitty_text: Some('a'),
        text: Some('a'),
        ..Row::new(Legacy, "\x1b[97::118u", Some("a"), &["a"])
    },
    Row {
        kitty_text: Some('a'),
        text: Some('a'),
        ..Row::new(Legacy, "\x1b[97::118;1u", Some("a"), &["a"])
    },
    Row {
        kitty_text: Some('a'),
        text: Some('a'),
        ..Row::new(Legacy, "\x1b[97::118;2u", Some("shift+a"), &["shift+a"])
    },
    Row::new(Legacy, "\x1b[97::118;5u", Some("ctrl+a"), &["ctrl+a"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[97::118;6:2u",
            Some("shift+ctrl+a"),
            &["shift+ctrl+a"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[97::118;5:3u", Some("ctrl+a"), &["ctrl+a"])
    },
    Row {
        kitty_text: Some('c'),
        text: Some('c'),
        ..Row::new(Legacy, "\x1b[99u", Some("c"), &["c"])
    },
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[99;6:2u",
            Some("shift+ctrl+c"),
            &["ctrl+shift+c", "shift+ctrl+c"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[99;5:3u", Some("ctrl+c"), &["ctrl+c"])
    },
    Row {
        kitty_text: Some('c'),
        text: Some('c'),
        ..Row::new(Legacy, "\x1b[99:u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('c'),
        text: Some('c'),
        ..Row::new(Legacy, "\x1b[99:;1u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('c'),
        text: Some('c'),
        ..Row::new(Legacy, "\x1b[99:;2u", Some("shift+c"), &["shift+c"])
    },
    Row::new(Legacy, "\x1b[99:;5u", Some("ctrl+c"), &["ctrl+c"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[99:;6:2u",
            Some("shift+ctrl+c"),
            &["ctrl+shift+c", "shift+ctrl+c"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[99:;5:3u", Some("ctrl+c"), &["ctrl+c"])
    },
    Row {
        kitty_text: Some('c'),
        text: Some('c'),
        ..Row::new(Legacy, "\x1b[99:67u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('c'),
        text: Some('c'),
        ..Row::new(Legacy, "\x1b[99:67;1u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('C'),
        text: Some('C'),
        ..Row::new(Legacy, "\x1b[99:67;2u", Some("shift+c"), &["shift+c"])
    },
    Row::new(Legacy, "\x1b[99:67;5u", Some("ctrl+c"), &["ctrl+c"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[99:67;6:2u",
            Some("shift+ctrl+c"),
            &["ctrl+shift+c", "shift+ctrl+c"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[99:67;5:3u", Some("ctrl+c"), &["ctrl+c"])
    },
    Row {
        kitty_text: Some('c'),
        text: Some('c'),
        ..Row::new(Legacy, "\x1b[99::99u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('c'),
        text: Some('c'),
        ..Row::new(Legacy, "\x1b[99::99;1u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('c'),
        text: Some('c'),
        ..Row::new(Legacy, "\x1b[99::99;2u", Some("shift+c"), &["shift+c"])
    },
    Row::new(Legacy, "\x1b[99::99;5u", Some("ctrl+c"), &["ctrl+c"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[99::99;6:2u",
            Some("shift+ctrl+c"),
            &["ctrl+shift+c", "shift+ctrl+c"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[99::99;5:3u", Some("ctrl+c"), &["ctrl+c"])
    },
    Row {
        kitty_text: Some('c'),
        text: Some('c'),
        ..Row::new(Legacy, "\x1b[99:67:99u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('c'),
        text: Some('c'),
        ..Row::new(Legacy, "\x1b[99:67:99;1u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('C'),
        text: Some('C'),
        ..Row::new(Legacy, "\x1b[99:67:99;2u", Some("shift+c"), &["shift+c"])
    },
    Row::new(Legacy, "\x1b[99:67:99;5u", Some("ctrl+c"), &["ctrl+c"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[99:67:99;6:2u",
            Some("shift+ctrl+c"),
            &["ctrl+shift+c", "shift+ctrl+c"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[99:67:99;5:3u", Some("ctrl+c"), &["ctrl+c"])
    },
    Row {
        kitty_text: Some('c'),
        text: Some('c'),
        ..Row::new(Legacy, "\x1b[99::118u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('c'),
        text: Some('c'),
        ..Row::new(Legacy, "\x1b[99::118;1u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('c'),
        text: Some('c'),
        ..Row::new(Legacy, "\x1b[99::118;2u", Some("shift+c"), &["shift+c"])
    },
    Row::new(Legacy, "\x1b[99::118;5u", Some("ctrl+c"), &["ctrl+c"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[99::118;6:2u",
            Some("shift+ctrl+c"),
            &["ctrl+shift+c", "shift+ctrl+c"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[99::118;5:3u", Some("ctrl+c"), &["ctrl+c"])
    },
    Row {
        kitty_text: Some('/'),
        text: Some('/'),
        ..Row::new(Legacy, "\x1b[47u", Some("/"), &["/"])
    },
    Row {
        kitty_text: Some('/'),
        text: Some('/'),
        ..Row::new(Legacy, "\x1b[47;1u", Some("/"), &["/"])
    },
    Row {
        kitty_text: Some('/'),
        text: Some('/'),
        ..Row::new(Legacy, "\x1b[47;2u", Some("shift+/"), &["shift+/"])
    },
    Row::new(Legacy, "\x1b[47;5u", Some("ctrl+/"), &["ctrl+/"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[47;6:2u",
            Some("shift+ctrl+/"),
            &["shift+ctrl+/"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[47;5:3u", Some("ctrl+/"), &["ctrl+/"])
    },
    Row {
        kitty_text: Some('/'),
        text: Some('/'),
        ..Row::new(Legacy, "\x1b[47:u", Some("/"), &["/"])
    },
    Row {
        kitty_text: Some('/'),
        text: Some('/'),
        ..Row::new(Legacy, "\x1b[47:;1u", Some("/"), &["/"])
    },
    Row {
        kitty_text: Some('/'),
        text: Some('/'),
        ..Row::new(Legacy, "\x1b[47:;2u", Some("shift+/"), &["shift+/"])
    },
    Row::new(Legacy, "\x1b[47:;5u", Some("ctrl+/"), &["ctrl+/"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[47:;6:2u",
            Some("shift+ctrl+/"),
            &["shift+ctrl+/"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[47:;5:3u", Some("ctrl+/"), &["ctrl+/"])
    },
    Row {
        kitty_text: Some('/'),
        text: Some('/'),
        ..Row::new(Legacy, "\x1b[47:67u", Some("/"), &["/"])
    },
    Row {
        kitty_text: Some('/'),
        text: Some('/'),
        ..Row::new(Legacy, "\x1b[47:67;1u", Some("/"), &["/"])
    },
    Row {
        kitty_text: Some('C'),
        text: Some('C'),
        ..Row::new(Legacy, "\x1b[47:67;2u", Some("shift+/"), &["shift+/"])
    },
    Row::new(Legacy, "\x1b[47:67;5u", Some("ctrl+/"), &["ctrl+/"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[47:67;6:2u",
            Some("shift+ctrl+/"),
            &["shift+ctrl+/"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[47:67;5:3u", Some("ctrl+/"), &["ctrl+/"])
    },
    Row {
        kitty_text: Some('/'),
        text: Some('/'),
        ..Row::new(Legacy, "\x1b[47::99u", Some("/"), &["/"])
    },
    Row {
        kitty_text: Some('/'),
        text: Some('/'),
        ..Row::new(Legacy, "\x1b[47::99;1u", Some("/"), &["/"])
    },
    Row {
        kitty_text: Some('/'),
        text: Some('/'),
        ..Row::new(Legacy, "\x1b[47::99;2u", Some("shift+/"), &["shift+/"])
    },
    Row::new(Legacy, "\x1b[47::99;5u", Some("ctrl+/"), &["ctrl+/"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[47::99;6:2u",
            Some("shift+ctrl+/"),
            &["shift+ctrl+/"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[47::99;5:3u", Some("ctrl+/"), &["ctrl+/"])
    },
    Row {
        kitty_text: Some('/'),
        text: Some('/'),
        ..Row::new(Legacy, "\x1b[47:67:99u", Some("/"), &["/"])
    },
    Row {
        kitty_text: Some('/'),
        text: Some('/'),
        ..Row::new(Legacy, "\x1b[47:67:99;1u", Some("/"), &["/"])
    },
    Row {
        kitty_text: Some('C'),
        text: Some('C'),
        ..Row::new(Legacy, "\x1b[47:67:99;2u", Some("shift+/"), &["shift+/"])
    },
    Row::new(Legacy, "\x1b[47:67:99;5u", Some("ctrl+/"), &["ctrl+/"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[47:67:99;6:2u",
            Some("shift+ctrl+/"),
            &["shift+ctrl+/"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[47:67:99;5:3u", Some("ctrl+/"), &["ctrl+/"])
    },
    Row {
        kitty_text: Some('/'),
        text: Some('/'),
        ..Row::new(Legacy, "\x1b[47::118u", Some("/"), &["/"])
    },
    Row {
        kitty_text: Some('/'),
        text: Some('/'),
        ..Row::new(Legacy, "\x1b[47::118;1u", Some("/"), &["/"])
    },
    Row {
        kitty_text: Some('/'),
        text: Some('/'),
        ..Row::new(Legacy, "\x1b[47::118;2u", Some("shift+/"), &["shift+/"])
    },
    Row::new(Legacy, "\x1b[47::118;5u", Some("ctrl+/"), &["ctrl+/"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[47::118;6:2u",
            Some("shift+ctrl+/"),
            &["shift+ctrl+/"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[47::118;5:3u", Some("ctrl+/"), &["ctrl+/"])
    },
    Row {
        kitty_text: Some('\u{441}'),
        text: Some('\u{441}'),
        ..Row::new(Legacy, "\x1b[1089u", None, &[])
    },
    Row {
        kitty_text: Some('\u{441}'),
        text: Some('\u{441}'),
        ..Row::new(Legacy, "\x1b[1089;1u", None, &[])
    },
    Row {
        kitty_text: Some('\u{441}'),
        text: Some('\u{441}'),
        ..Row::new(Legacy, "\x1b[1089;2u", None, &[])
    },
    Row::new(Legacy, "\x1b[1089;5u", None, &[]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(Legacy, "\x1b[1089;6:2u", None, &[])
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[1089;5:3u", None, &[])
    },
    Row {
        kitty_text: Some('\u{441}'),
        text: Some('\u{441}'),
        ..Row::new(Legacy, "\x1b[1089:u", None, &[])
    },
    Row {
        kitty_text: Some('\u{441}'),
        text: Some('\u{441}'),
        ..Row::new(Legacy, "\x1b[1089:;1u", None, &[])
    },
    Row {
        kitty_text: Some('\u{441}'),
        text: Some('\u{441}'),
        ..Row::new(Legacy, "\x1b[1089:;2u", None, &[])
    },
    Row::new(Legacy, "\x1b[1089:;5u", None, &[]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(Legacy, "\x1b[1089:;6:2u", None, &[])
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[1089:;5:3u", None, &[])
    },
    Row {
        kitty_text: Some('\u{441}'),
        text: Some('\u{441}'),
        ..Row::new(Legacy, "\x1b[1089:67u", None, &[])
    },
    Row {
        kitty_text: Some('\u{441}'),
        text: Some('\u{441}'),
        ..Row::new(Legacy, "\x1b[1089:67;1u", None, &[])
    },
    Row {
        kitty_text: Some('C'),
        text: Some('C'),
        ..Row::new(Legacy, "\x1b[1089:67;2u", None, &[])
    },
    Row::new(Legacy, "\x1b[1089:67;5u", None, &[]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(Legacy, "\x1b[1089:67;6:2u", None, &[])
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[1089:67;5:3u", None, &[])
    },
    Row {
        kitty_text: Some('\u{441}'),
        text: Some('\u{441}'),
        ..Row::new(Legacy, "\x1b[1089::99u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('\u{441}'),
        text: Some('\u{441}'),
        ..Row::new(Legacy, "\x1b[1089::99;1u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('\u{441}'),
        text: Some('\u{441}'),
        ..Row::new(Legacy, "\x1b[1089::99;2u", Some("shift+c"), &["shift+c"])
    },
    Row::new(Legacy, "\x1b[1089::99;5u", Some("ctrl+c"), &["ctrl+c"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[1089::99;6:2u",
            Some("shift+ctrl+c"),
            &["ctrl+shift+c", "shift+ctrl+c"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[1089::99;5:3u", Some("ctrl+c"), &["ctrl+c"])
    },
    Row {
        kitty_text: Some('\u{441}'),
        text: Some('\u{441}'),
        ..Row::new(Legacy, "\x1b[1089:67:99u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('\u{441}'),
        text: Some('\u{441}'),
        ..Row::new(Legacy, "\x1b[1089:67:99;1u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('C'),
        text: Some('C'),
        ..Row::new(Legacy, "\x1b[1089:67:99;2u", Some("shift+c"), &["shift+c"])
    },
    Row::new(Legacy, "\x1b[1089:67:99;5u", Some("ctrl+c"), &["ctrl+c"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[1089:67:99;6:2u",
            Some("shift+ctrl+c"),
            &["ctrl+shift+c", "shift+ctrl+c"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[1089:67:99;5:3u", Some("ctrl+c"), &["ctrl+c"])
    },
    Row {
        kitty_text: Some('\u{441}'),
        text: Some('\u{441}'),
        ..Row::new(Legacy, "\x1b[1089::118u", Some("v"), &["v"])
    },
    Row {
        kitty_text: Some('\u{441}'),
        text: Some('\u{441}'),
        ..Row::new(Legacy, "\x1b[1089::118;1u", Some("v"), &["v"])
    },
    Row {
        kitty_text: Some('\u{441}'),
        text: Some('\u{441}'),
        ..Row::new(Legacy, "\x1b[1089::118;2u", Some("shift+v"), &["shift+v"])
    },
    Row::new(Legacy, "\x1b[1089::118;5u", Some("ctrl+v"), &["ctrl+v"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[1089::118;6:2u",
            Some("shift+ctrl+v"),
            &["shift+ctrl+v"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[1089::118;5:3u", Some("ctrl+v"), &["ctrl+v"])
    },
    Row {
        kitty_text: Some('\u{432}'),
        text: Some('\u{432}'),
        ..Row::new(Legacy, "\x1b[1074u", None, &[])
    },
    Row {
        kitty_text: Some('\u{432}'),
        text: Some('\u{432}'),
        ..Row::new(Legacy, "\x1b[1074;1u", None, &[])
    },
    Row {
        kitty_text: Some('\u{432}'),
        text: Some('\u{432}'),
        ..Row::new(Legacy, "\x1b[1074;2u", None, &[])
    },
    Row::new(Legacy, "\x1b[1074;5u", None, &[]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(Legacy, "\x1b[1074;6:2u", None, &[])
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[1074;5:3u", None, &[])
    },
    Row {
        kitty_text: Some('\u{432}'),
        text: Some('\u{432}'),
        ..Row::new(Legacy, "\x1b[1074:u", None, &[])
    },
    Row {
        kitty_text: Some('\u{432}'),
        text: Some('\u{432}'),
        ..Row::new(Legacy, "\x1b[1074:;1u", None, &[])
    },
    Row {
        kitty_text: Some('\u{432}'),
        text: Some('\u{432}'),
        ..Row::new(Legacy, "\x1b[1074:;2u", None, &[])
    },
    Row::new(Legacy, "\x1b[1074:;5u", None, &[]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(Legacy, "\x1b[1074:;6:2u", None, &[])
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[1074:;5:3u", None, &[])
    },
    Row {
        kitty_text: Some('\u{432}'),
        text: Some('\u{432}'),
        ..Row::new(Legacy, "\x1b[1074:67u", None, &[])
    },
    Row {
        kitty_text: Some('\u{432}'),
        text: Some('\u{432}'),
        ..Row::new(Legacy, "\x1b[1074:67;1u", None, &[])
    },
    Row {
        kitty_text: Some('C'),
        text: Some('C'),
        ..Row::new(Legacy, "\x1b[1074:67;2u", None, &[])
    },
    Row::new(Legacy, "\x1b[1074:67;5u", None, &[]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(Legacy, "\x1b[1074:67;6:2u", None, &[])
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[1074:67;5:3u", None, &[])
    },
    Row {
        kitty_text: Some('\u{432}'),
        text: Some('\u{432}'),
        ..Row::new(Legacy, "\x1b[1074::99u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('\u{432}'),
        text: Some('\u{432}'),
        ..Row::new(Legacy, "\x1b[1074::99;1u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('\u{432}'),
        text: Some('\u{432}'),
        ..Row::new(Legacy, "\x1b[1074::99;2u", Some("shift+c"), &["shift+c"])
    },
    Row::new(Legacy, "\x1b[1074::99;5u", Some("ctrl+c"), &["ctrl+c"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[1074::99;6:2u",
            Some("shift+ctrl+c"),
            &["ctrl+shift+c", "shift+ctrl+c"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[1074::99;5:3u", Some("ctrl+c"), &["ctrl+c"])
    },
    Row {
        kitty_text: Some('\u{432}'),
        text: Some('\u{432}'),
        ..Row::new(Legacy, "\x1b[1074:67:99u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('\u{432}'),
        text: Some('\u{432}'),
        ..Row::new(Legacy, "\x1b[1074:67:99;1u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('C'),
        text: Some('C'),
        ..Row::new(Legacy, "\x1b[1074:67:99;2u", Some("shift+c"), &["shift+c"])
    },
    Row::new(Legacy, "\x1b[1074:67:99;5u", Some("ctrl+c"), &["ctrl+c"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[1074:67:99;6:2u",
            Some("shift+ctrl+c"),
            &["ctrl+shift+c", "shift+ctrl+c"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[1074:67:99;5:3u", Some("ctrl+c"), &["ctrl+c"])
    },
    Row {
        kitty_text: Some('\u{432}'),
        text: Some('\u{432}'),
        ..Row::new(Legacy, "\x1b[1074::118u", Some("v"), &["v"])
    },
    Row {
        kitty_text: Some('\u{432}'),
        text: Some('\u{432}'),
        ..Row::new(Legacy, "\x1b[1074::118;1u", Some("v"), &["v"])
    },
    Row {
        kitty_text: Some('\u{432}'),
        text: Some('\u{432}'),
        ..Row::new(Legacy, "\x1b[1074::118;2u", Some("shift+v"), &["shift+v"])
    },
    Row::new(Legacy, "\x1b[1074::118;5u", Some("ctrl+v"), &["ctrl+v"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[1074::118;6:2u",
            Some("shift+ctrl+v"),
            &["shift+ctrl+v"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[1074::118;5:3u", Some("ctrl+v"), &["ctrl+v"])
    },
    Row {
        kitty_text: Some('\u{44f}'),
        text: Some('\u{44f}'),
        ..Row::new(Legacy, "\x1b[1103u", None, &[])
    },
    Row {
        kitty_text: Some('\u{44f}'),
        text: Some('\u{44f}'),
        ..Row::new(Legacy, "\x1b[1103;1u", None, &[])
    },
    Row {
        kitty_text: Some('\u{44f}'),
        text: Some('\u{44f}'),
        ..Row::new(Legacy, "\x1b[1103;2u", None, &[])
    },
    Row::new(Legacy, "\x1b[1103;5u", None, &[]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(Legacy, "\x1b[1103;6:2u", None, &[])
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[1103;5:3u", None, &[])
    },
    Row {
        kitty_text: Some('\u{44f}'),
        text: Some('\u{44f}'),
        ..Row::new(Legacy, "\x1b[1103:u", None, &[])
    },
    Row {
        kitty_text: Some('\u{44f}'),
        text: Some('\u{44f}'),
        ..Row::new(Legacy, "\x1b[1103:;1u", None, &[])
    },
    Row {
        kitty_text: Some('\u{44f}'),
        text: Some('\u{44f}'),
        ..Row::new(Legacy, "\x1b[1103:;2u", None, &[])
    },
    Row::new(Legacy, "\x1b[1103:;5u", None, &[]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(Legacy, "\x1b[1103:;6:2u", None, &[])
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[1103:;5:3u", None, &[])
    },
    Row {
        kitty_text: Some('\u{44f}'),
        text: Some('\u{44f}'),
        ..Row::new(Legacy, "\x1b[1103:67u", None, &[])
    },
    Row {
        kitty_text: Some('\u{44f}'),
        text: Some('\u{44f}'),
        ..Row::new(Legacy, "\x1b[1103:67;1u", None, &[])
    },
    Row {
        kitty_text: Some('C'),
        text: Some('C'),
        ..Row::new(Legacy, "\x1b[1103:67;2u", None, &[])
    },
    Row::new(Legacy, "\x1b[1103:67;5u", None, &[]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(Legacy, "\x1b[1103:67;6:2u", None, &[])
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[1103:67;5:3u", None, &[])
    },
    Row {
        kitty_text: Some('\u{44f}'),
        text: Some('\u{44f}'),
        ..Row::new(Legacy, "\x1b[1103::99u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('\u{44f}'),
        text: Some('\u{44f}'),
        ..Row::new(Legacy, "\x1b[1103::99;1u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('\u{44f}'),
        text: Some('\u{44f}'),
        ..Row::new(Legacy, "\x1b[1103::99;2u", Some("shift+c"), &["shift+c"])
    },
    Row::new(Legacy, "\x1b[1103::99;5u", Some("ctrl+c"), &["ctrl+c"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[1103::99;6:2u",
            Some("shift+ctrl+c"),
            &["ctrl+shift+c", "shift+ctrl+c"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[1103::99;5:3u", Some("ctrl+c"), &["ctrl+c"])
    },
    Row {
        kitty_text: Some('\u{44f}'),
        text: Some('\u{44f}'),
        ..Row::new(Legacy, "\x1b[1103:67:99u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('\u{44f}'),
        text: Some('\u{44f}'),
        ..Row::new(Legacy, "\x1b[1103:67:99;1u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('C'),
        text: Some('C'),
        ..Row::new(Legacy, "\x1b[1103:67:99;2u", Some("shift+c"), &["shift+c"])
    },
    Row::new(Legacy, "\x1b[1103:67:99;5u", Some("ctrl+c"), &["ctrl+c"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[1103:67:99;6:2u",
            Some("shift+ctrl+c"),
            &["ctrl+shift+c", "shift+ctrl+c"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[1103:67:99;5:3u", Some("ctrl+c"), &["ctrl+c"])
    },
    Row {
        kitty_text: Some('\u{44f}'),
        text: Some('\u{44f}'),
        ..Row::new(Legacy, "\x1b[1103::118u", Some("v"), &["v"])
    },
    Row {
        kitty_text: Some('\u{44f}'),
        text: Some('\u{44f}'),
        ..Row::new(Legacy, "\x1b[1103::118;1u", Some("v"), &["v"])
    },
    Row {
        kitty_text: Some('\u{44f}'),
        text: Some('\u{44f}'),
        ..Row::new(Legacy, "\x1b[1103::118;2u", Some("shift+v"), &["shift+v"])
    },
    Row::new(Legacy, "\x1b[1103::118;5u", Some("ctrl+v"), &["ctrl+v"]),
    Row {
        event: KeyEventType::Repeat,
        ..Row::new(
            Legacy,
            "\x1b[1103::118;6:2u",
            Some("shift+ctrl+v"),
            &["shift+ctrl+v"],
        )
    },
    Row {
        event: KeyEventType::Release,
        ..Row::new(Legacy, "\x1b[1103::118;5:3u", Some("ctrl+v"), &["ctrl+v"])
    },
];

/// Candidate identifiers tried against every row of [`MODIFY_OTHER_KEYS`].
pub const MOK_CAND: &[&str] = &[
    "escape",
    "esc",
    "enter",
    "return",
    "tab",
    "space",
    "backspace",
    "ctrl+backspace",
    "ctrl+h",
    "ctrl+space",
    "shift+enter",
    "alt+enter",
    "shift+tab",
    "up",
    "down",
    "left",
    "right",
    "home",
    "end",
    "insert",
    "delete",
    "pageUp",
    "pageDown",
    "clear",
    "f1",
    "f12",
    "a",
    "shift+a",
    "ctrl+a",
    "alt+a",
    "ctrl+alt+a",
    "ctrl+c",
    "ctrl+1",
    "shift+1",
    "shift+e",
    "ctrl+shift+e",
    "ctrl+/",
    "super+a",
];

/// Rows of the `mok` probe.
pub const MODIFY_OTHER_KEYS: &[Row] = &[
    Row::new(Legacy, "\x1b[27;0;0~", None, &[]),
    Row::new(Legacy, "\x1b[27;1;0~", None, &[]),
    Row::new(Legacy, "\x1b[27;2;0~", None, &[]),
    Row::new(Legacy, "\x1b[27;3;0~", None, &[]),
    Row::new(Legacy, "\x1b[27;5;0~", None, &[]),
    Row::new(Legacy, "\x1b[27;6;0~", None, &[]),
    Row::new(Legacy, "\x1b[27;7;0~", None, &[]),
    Row::new(Legacy, "\x1b[27;9;0~", None, &[]),
    Row::new(Legacy, "\x1b[27;16;0~", None, &[]),
    Row::new(Legacy, "\x1b[27;17;0~", None, &[]),
    Row::new(Legacy, "\x1b[27;65;0~", None, &[]),
    Row::new(Legacy, "\x1b[27;66;0~", None, &[]),
    Row::new(Legacy, "\x1b[27;193;0~", None, &[]),
    Row::new(Legacy, "\x1b[27;194;0~", None, &[]),
    Row::new(Legacy, "\x1b[27;0;9~", None, &[]),
    Row::new(Legacy, "\x1b[27;1;9~", Some("tab"), &[]),
    Row::new(Legacy, "\x1b[27;2;9~", Some("shift+tab"), &["shift+tab"]),
    Row::new(Legacy, "\x1b[27;3;9~", Some("alt+tab"), &["alt+tab"]),
    Row::new(Legacy, "\x1b[27;5;9~", Some("ctrl+tab"), &["ctrl+tab"]),
    Row::new(
        Legacy,
        "\x1b[27;6;9~",
        Some("shift+ctrl+tab"),
        &["shift+ctrl+tab"],
    ),
    Row::new(
        Legacy,
        "\x1b[27;7;9~",
        Some("ctrl+alt+tab"),
        &["ctrl+alt+tab"],
    ),
    Row::new(Legacy, "\x1b[27;9;9~", Some("super+tab"), &["super+tab"]),
    Row::new(
        Legacy,
        "\x1b[27;16;9~",
        Some("shift+ctrl+alt+super+tab"),
        &["shift+ctrl+alt+super+tab"],
    ),
    Row::new(Legacy, "\x1b[27;17;9~", None, &[]),
    Row::new(Legacy, "\x1b[27;65;9~", Some("tab"), &[]),
    Row::new(Legacy, "\x1b[27;66;9~", Some("shift+tab"), &[]),
    Row::new(Legacy, "\x1b[27;193;9~", Some("tab"), &[]),
    Row::new(Legacy, "\x1b[27;194;9~", Some("shift+tab"), &[]),
    Row::new(Legacy, "\x1b[27;0;13~", None, &[]),
    Row::new(Legacy, "\x1b[27;1;13~", Some("enter"), &[]),
    Row::new(
        Legacy,
        "\x1b[27;2;13~",
        Some("shift+enter"),
        &["shift+enter"],
    ),
    Row::new(Legacy, "\x1b[27;3;13~", Some("alt+enter"), &["alt+enter"]),
    Row::new(Legacy, "\x1b[27;5;13~", Some("ctrl+enter"), &["ctrl+enter"]),
    Row::new(
        Legacy,
        "\x1b[27;6;13~",
        Some("shift+ctrl+enter"),
        &["shift+ctrl+enter"],
    ),
    Row::new(
        Legacy,
        "\x1b[27;7;13~",
        Some("ctrl+alt+enter"),
        &["ctrl+alt+enter"],
    ),
    Row::new(
        Legacy,
        "\x1b[27;9;13~",
        Some("super+enter"),
        &["super+enter"],
    ),
    Row::new(
        Legacy,
        "\x1b[27;16;13~",
        Some("shift+ctrl+alt+super+enter"),
        &["shift+ctrl+alt+super+enter"],
    ),
    Row::new(Legacy, "\x1b[27;17;13~", None, &[]),
    Row::new(Legacy, "\x1b[27;65;13~", Some("enter"), &[]),
    Row::new(Legacy, "\x1b[27;66;13~", Some("shift+enter"), &[]),
    Row::new(Legacy, "\x1b[27;193;13~", Some("enter"), &[]),
    Row::new(Legacy, "\x1b[27;194;13~", Some("shift+enter"), &[]),
    Row::new(Legacy, "\x1b[27;0;27~", None, &[]),
    Row::new(Legacy, "\x1b[27;1;27~", Some("escape"), &["escape", "esc"]),
    Row::new(Legacy, "\x1b[27;2;27~", Some("shift+escape"), &[]),
    Row::new(Legacy, "\x1b[27;3;27~", Some("alt+escape"), &[]),
    Row::new(Legacy, "\x1b[27;5;27~", Some("ctrl+escape"), &[]),
    Row::new(Legacy, "\x1b[27;6;27~", Some("shift+ctrl+escape"), &[]),
    Row::new(Legacy, "\x1b[27;7;27~", Some("ctrl+alt+escape"), &[]),
    Row::new(Legacy, "\x1b[27;9;27~", Some("super+escape"), &[]),
    Row::new(
        Legacy,
        "\x1b[27;16;27~",
        Some("shift+ctrl+alt+super+escape"),
        &[],
    ),
    Row::new(Legacy, "\x1b[27;17;27~", None, &[]),
    Row::new(Legacy, "\x1b[27;65;27~", Some("escape"), &[]),
    Row::new(Legacy, "\x1b[27;66;27~", Some("shift+escape"), &[]),
    Row::new(Legacy, "\x1b[27;193;27~", Some("escape"), &[]),
    Row::new(Legacy, "\x1b[27;194;27~", Some("shift+escape"), &[]),
    Row::new(Legacy, "\x1b[27;0;31~", None, &[]),
    Row::new(Legacy, "\x1b[27;1;31~", None, &[]),
    Row::new(Legacy, "\x1b[27;2;31~", None, &[]),
    Row::new(Legacy, "\x1b[27;3;31~", None, &[]),
    Row::new(Legacy, "\x1b[27;5;31~", None, &[]),
    Row::new(Legacy, "\x1b[27;6;31~", None, &[]),
    Row::new(Legacy, "\x1b[27;7;31~", None, &[]),
    Row::new(Legacy, "\x1b[27;9;31~", None, &[]),
    Row::new(Legacy, "\x1b[27;16;31~", None, &[]),
    Row::new(Legacy, "\x1b[27;17;31~", None, &[]),
    Row::new(Legacy, "\x1b[27;65;31~", None, &[]),
    Row::new(Legacy, "\x1b[27;66;31~", None, &[]),
    Row::new(Legacy, "\x1b[27;193;31~", None, &[]),
    Row::new(Legacy, "\x1b[27;194;31~", None, &[]),
    Row::new(Legacy, "\x1b[27;0;32~", None, &[]),
    Row {
        text: Some(' '),
        ..Row::new(Legacy, "\x1b[27;1;32~", Some("space"), &["space"])
    },
    Row {
        text: Some(' '),
        ..Row::new(
            Legacy,
            "\x1b[27;2;32~",
            Some("shift+space"),
            &["shift+space"],
        )
    },
    Row::new(Legacy, "\x1b[27;3;32~", Some("alt+space"), &["alt+space"]),
    Row::new(Legacy, "\x1b[27;5;32~", Some("ctrl+space"), &["ctrl+space"]),
    Row::new(
        Legacy,
        "\x1b[27;6;32~",
        Some("shift+ctrl+space"),
        &["shift+ctrl+space"],
    ),
    Row::new(
        Legacy,
        "\x1b[27;7;32~",
        Some("ctrl+alt+space"),
        &["ctrl+alt+space"],
    ),
    Row::new(
        Legacy,
        "\x1b[27;9;32~",
        Some("super+space"),
        &["super+space"],
    ),
    Row::new(
        Legacy,
        "\x1b[27;16;32~",
        Some("shift+ctrl+alt+super+space"),
        &["shift+ctrl+alt+super+space"],
    ),
    Row::new(Legacy, "\x1b[27;17;32~", None, &[]),
    Row {
        text: Some(' '),
        ..Row::new(Legacy, "\x1b[27;65;32~", Some("space"), &[])
    },
    Row {
        text: Some(' '),
        ..Row::new(Legacy, "\x1b[27;66;32~", Some("shift+space"), &[])
    },
    Row {
        text: Some(' '),
        ..Row::new(Legacy, "\x1b[27;193;32~", Some("space"), &[])
    },
    Row {
        text: Some(' '),
        ..Row::new(Legacy, "\x1b[27;194;32~", Some("shift+space"), &[])
    },
    Row::new(Legacy, "\x1b[27;0;33~", None, &[]),
    Row {
        text: Some('!'),
        ..Row::new(Legacy, "\x1b[27;1;33~", Some("!"), &[])
    },
    Row {
        text: Some('!'),
        ..Row::new(Legacy, "\x1b[27;2;33~", Some("shift+!"), &["shift+!"])
    },
    Row::new(Legacy, "\x1b[27;3;33~", Some("alt+!"), &["alt+!"]),
    Row::new(Legacy, "\x1b[27;5;33~", Some("ctrl+!"), &["ctrl+!"]),
    Row::new(
        Legacy,
        "\x1b[27;6;33~",
        Some("shift+ctrl+!"),
        &["shift+ctrl+!"],
    ),
    Row::new(Legacy, "\x1b[27;7;33~", Some("ctrl+alt+!"), &["ctrl+alt+!"]),
    Row::new(Legacy, "\x1b[27;9;33~", Some("super+!"), &["super+!"]),
    Row::new(
        Legacy,
        "\x1b[27;16;33~",
        Some("shift+ctrl+alt+super+!"),
        &["shift+ctrl+alt+super+!"],
    ),
    Row::new(Legacy, "\x1b[27;17;33~", None, &[]),
    Row {
        text: Some('!'),
        ..Row::new(Legacy, "\x1b[27;65;33~", Some("!"), &[])
    },
    Row {
        text: Some('!'),
        ..Row::new(Legacy, "\x1b[27;66;33~", Some("shift+!"), &[])
    },
    Row {
        text: Some('!'),
        ..Row::new(Legacy, "\x1b[27;193;33~", Some("!"), &[])
    },
    Row {
        text: Some('!'),
        ..Row::new(Legacy, "\x1b[27;194;33~", Some("shift+!"), &[])
    },
    Row::new(Legacy, "\x1b[27;0;47~", None, &[]),
    Row {
        text: Some('/'),
        ..Row::new(Legacy, "\x1b[27;1;47~", Some("/"), &[])
    },
    Row {
        text: Some('/'),
        ..Row::new(Legacy, "\x1b[27;2;47~", Some("shift+/"), &["shift+/"])
    },
    Row::new(Legacy, "\x1b[27;3;47~", Some("alt+/"), &["alt+/"]),
    Row::new(Legacy, "\x1b[27;5;47~", Some("ctrl+/"), &["ctrl+/"]),
    Row::new(
        Legacy,
        "\x1b[27;6;47~",
        Some("shift+ctrl+/"),
        &["shift+ctrl+/"],
    ),
    Row::new(Legacy, "\x1b[27;7;47~", Some("ctrl+alt+/"), &["ctrl+alt+/"]),
    Row::new(Legacy, "\x1b[27;9;47~", Some("super+/"), &["super+/"]),
    Row::new(
        Legacy,
        "\x1b[27;16;47~",
        Some("shift+ctrl+alt+super+/"),
        &["shift+ctrl+alt+super+/"],
    ),
    Row::new(Legacy, "\x1b[27;17;47~", None, &[]),
    Row {
        text: Some('/'),
        ..Row::new(Legacy, "\x1b[27;65;47~", Some("/"), &[])
    },
    Row {
        text: Some('/'),
        ..Row::new(Legacy, "\x1b[27;66;47~", Some("shift+/"), &[])
    },
    Row {
        text: Some('/'),
        ..Row::new(Legacy, "\x1b[27;193;47~", Some("/"), &[])
    },
    Row {
        text: Some('/'),
        ..Row::new(Legacy, "\x1b[27;194;47~", Some("shift+/"), &[])
    },
    Row::new(Legacy, "\x1b[27;0;48~", None, &[]),
    Row {
        text: Some('0'),
        ..Row::new(Legacy, "\x1b[27;1;48~", Some("0"), &[])
    },
    Row {
        text: Some('0'),
        ..Row::new(Legacy, "\x1b[27;2;48~", Some("shift+0"), &["shift+0"])
    },
    Row::new(Legacy, "\x1b[27;3;48~", Some("alt+0"), &["alt+0"]),
    Row::new(Legacy, "\x1b[27;5;48~", Some("ctrl+0"), &["ctrl+0"]),
    Row::new(
        Legacy,
        "\x1b[27;6;48~",
        Some("shift+ctrl+0"),
        &["shift+ctrl+0"],
    ),
    Row::new(Legacy, "\x1b[27;7;48~", Some("ctrl+alt+0"), &["ctrl+alt+0"]),
    Row::new(Legacy, "\x1b[27;9;48~", Some("super+0"), &["super+0"]),
    Row::new(
        Legacy,
        "\x1b[27;16;48~",
        Some("shift+ctrl+alt+super+0"),
        &["shift+ctrl+alt+super+0"],
    ),
    Row::new(Legacy, "\x1b[27;17;48~", None, &[]),
    Row {
        text: Some('0'),
        ..Row::new(Legacy, "\x1b[27;65;48~", Some("0"), &[])
    },
    Row {
        text: Some('0'),
        ..Row::new(Legacy, "\x1b[27;66;48~", Some("shift+0"), &[])
    },
    Row {
        text: Some('0'),
        ..Row::new(Legacy, "\x1b[27;193;48~", Some("0"), &[])
    },
    Row {
        text: Some('0'),
        ..Row::new(Legacy, "\x1b[27;194;48~", Some("shift+0"), &[])
    },
    Row::new(Legacy, "\x1b[27;0;49~", None, &[]),
    Row {
        text: Some('1'),
        ..Row::new(Legacy, "\x1b[27;1;49~", Some("1"), &[])
    },
    Row {
        text: Some('1'),
        ..Row::new(Legacy, "\x1b[27;2;49~", Some("shift+1"), &["shift+1"])
    },
    Row::new(Legacy, "\x1b[27;3;49~", Some("alt+1"), &["alt+1"]),
    Row::new(Legacy, "\x1b[27;5;49~", Some("ctrl+1"), &["ctrl+1"]),
    Row::new(
        Legacy,
        "\x1b[27;6;49~",
        Some("shift+ctrl+1"),
        &["shift+ctrl+1"],
    ),
    Row::new(Legacy, "\x1b[27;7;49~", Some("ctrl+alt+1"), &["ctrl+alt+1"]),
    Row::new(Legacy, "\x1b[27;9;49~", Some("super+1"), &["super+1"]),
    Row::new(
        Legacy,
        "\x1b[27;16;49~",
        Some("shift+ctrl+alt+super+1"),
        &["shift+ctrl+alt+super+1"],
    ),
    Row::new(Legacy, "\x1b[27;17;49~", None, &[]),
    Row {
        text: Some('1'),
        ..Row::new(Legacy, "\x1b[27;65;49~", Some("1"), &[])
    },
    Row {
        text: Some('1'),
        ..Row::new(Legacy, "\x1b[27;66;49~", Some("shift+1"), &[])
    },
    Row {
        text: Some('1'),
        ..Row::new(Legacy, "\x1b[27;193;49~", Some("1"), &[])
    },
    Row {
        text: Some('1'),
        ..Row::new(Legacy, "\x1b[27;194;49~", Some("shift+1"), &[])
    },
    Row::new(Legacy, "\x1b[27;0;65~", None, &[]),
    Row {
        text: Some('A'),
        ..Row::new(Legacy, "\x1b[27;1;65~", None, &[])
    },
    Row {
        text: Some('A'),
        ..Row::new(Legacy, "\x1b[27;2;65~", Some("shift+a"), &["shift+a"])
    },
    Row::new(Legacy, "\x1b[27;3;65~", None, &[]),
    Row::new(Legacy, "\x1b[27;5;65~", None, &[]),
    Row::new(
        Legacy,
        "\x1b[27;6;65~",
        Some("shift+ctrl+a"),
        &["shift+ctrl+a"],
    ),
    Row::new(Legacy, "\x1b[27;7;65~", None, &[]),
    Row::new(Legacy, "\x1b[27;9;65~", None, &[]),
    Row::new(
        Legacy,
        "\x1b[27;16;65~",
        Some("shift+ctrl+alt+super+a"),
        &["shift+ctrl+alt+super+a"],
    ),
    Row::new(Legacy, "\x1b[27;17;65~", None, &[]),
    Row {
        text: Some('A'),
        ..Row::new(Legacy, "\x1b[27;65;65~", None, &[])
    },
    Row {
        text: Some('A'),
        ..Row::new(Legacy, "\x1b[27;66;65~", Some("shift+a"), &[])
    },
    Row {
        text: Some('A'),
        ..Row::new(Legacy, "\x1b[27;193;65~", None, &[])
    },
    Row {
        text: Some('A'),
        ..Row::new(Legacy, "\x1b[27;194;65~", Some("shift+a"), &[])
    },
    Row::new(Legacy, "\x1b[27;0;69~", None, &[]),
    Row {
        text: Some('E'),
        ..Row::new(Legacy, "\x1b[27;1;69~", None, &[])
    },
    Row {
        text: Some('E'),
        ..Row::new(Legacy, "\x1b[27;2;69~", Some("shift+e"), &["shift+e"])
    },
    Row::new(Legacy, "\x1b[27;3;69~", None, &[]),
    Row::new(Legacy, "\x1b[27;5;69~", None, &[]),
    Row::new(
        Legacy,
        "\x1b[27;6;69~",
        Some("shift+ctrl+e"),
        &["ctrl+shift+e", "shift+ctrl+e"],
    ),
    Row::new(Legacy, "\x1b[27;7;69~", None, &[]),
    Row::new(Legacy, "\x1b[27;9;69~", None, &[]),
    Row::new(
        Legacy,
        "\x1b[27;16;69~",
        Some("shift+ctrl+alt+super+e"),
        &["shift+ctrl+alt+super+e"],
    ),
    Row::new(Legacy, "\x1b[27;17;69~", None, &[]),
    Row {
        text: Some('E'),
        ..Row::new(Legacy, "\x1b[27;65;69~", None, &[])
    },
    Row {
        text: Some('E'),
        ..Row::new(Legacy, "\x1b[27;66;69~", Some("shift+e"), &[])
    },
    Row {
        text: Some('E'),
        ..Row::new(Legacy, "\x1b[27;193;69~", None, &[])
    },
    Row {
        text: Some('E'),
        ..Row::new(Legacy, "\x1b[27;194;69~", Some("shift+e"), &[])
    },
    Row::new(Legacy, "\x1b[27;0;90~", None, &[]),
    Row {
        text: Some('Z'),
        ..Row::new(Legacy, "\x1b[27;1;90~", None, &[])
    },
    Row {
        text: Some('Z'),
        ..Row::new(Legacy, "\x1b[27;2;90~", Some("shift+z"), &["shift+z"])
    },
    Row::new(Legacy, "\x1b[27;3;90~", None, &[]),
    Row::new(Legacy, "\x1b[27;5;90~", None, &[]),
    Row::new(
        Legacy,
        "\x1b[27;6;90~",
        Some("shift+ctrl+z"),
        &["shift+ctrl+z"],
    ),
    Row::new(Legacy, "\x1b[27;7;90~", None, &[]),
    Row::new(Legacy, "\x1b[27;9;90~", None, &[]),
    Row::new(
        Legacy,
        "\x1b[27;16;90~",
        Some("shift+ctrl+alt+super+z"),
        &["shift+ctrl+alt+super+z"],
    ),
    Row::new(Legacy, "\x1b[27;17;90~", None, &[]),
    Row {
        text: Some('Z'),
        ..Row::new(Legacy, "\x1b[27;65;90~", None, &[])
    },
    Row {
        text: Some('Z'),
        ..Row::new(Legacy, "\x1b[27;66;90~", Some("shift+z"), &[])
    },
    Row {
        text: Some('Z'),
        ..Row::new(Legacy, "\x1b[27;193;90~", None, &[])
    },
    Row {
        text: Some('Z'),
        ..Row::new(Legacy, "\x1b[27;194;90~", Some("shift+z"), &[])
    },
    Row::new(Legacy, "\x1b[27;0;97~", None, &[]),
    Row {
        text: Some('a'),
        ..Row::new(Legacy, "\x1b[27;1;97~", Some("a"), &[])
    },
    Row {
        text: Some('a'),
        ..Row::new(Legacy, "\x1b[27;2;97~", Some("shift+a"), &["shift+a"])
    },
    Row::new(Legacy, "\x1b[27;3;97~", Some("alt+a"), &["alt+a"]),
    Row::new(Legacy, "\x1b[27;5;97~", Some("ctrl+a"), &["ctrl+a"]),
    Row::new(
        Legacy,
        "\x1b[27;6;97~",
        Some("shift+ctrl+a"),
        &["shift+ctrl+a"],
    ),
    Row::new(Legacy, "\x1b[27;7;97~", Some("ctrl+alt+a"), &["ctrl+alt+a"]),
    Row::new(Legacy, "\x1b[27;9;97~", Some("super+a"), &["super+a"]),
    Row::new(
        Legacy,
        "\x1b[27;16;97~",
        Some("shift+ctrl+alt+super+a"),
        &["shift+ctrl+alt+super+a"],
    ),
    Row::new(Legacy, "\x1b[27;17;97~", None, &[]),
    Row {
        text: Some('a'),
        ..Row::new(Legacy, "\x1b[27;65;97~", Some("a"), &[])
    },
    Row {
        text: Some('a'),
        ..Row::new(Legacy, "\x1b[27;66;97~", Some("shift+a"), &[])
    },
    Row {
        text: Some('a'),
        ..Row::new(Legacy, "\x1b[27;193;97~", Some("a"), &[])
    },
    Row {
        text: Some('a'),
        ..Row::new(Legacy, "\x1b[27;194;97~", Some("shift+a"), &[])
    },
    Row::new(Legacy, "\x1b[27;0;99~", None, &[]),
    Row {
        text: Some('c'),
        ..Row::new(Legacy, "\x1b[27;1;99~", Some("c"), &[])
    },
    Row {
        text: Some('c'),
        ..Row::new(Legacy, "\x1b[27;2;99~", Some("shift+c"), &["shift+c"])
    },
    Row::new(Legacy, "\x1b[27;3;99~", Some("alt+c"), &["alt+c"]),
    Row::new(Legacy, "\x1b[27;5;99~", Some("ctrl+c"), &["ctrl+c"]),
    Row::new(
        Legacy,
        "\x1b[27;6;99~",
        Some("shift+ctrl+c"),
        &["shift+ctrl+c"],
    ),
    Row::new(Legacy, "\x1b[27;7;99~", Some("ctrl+alt+c"), &["ctrl+alt+c"]),
    Row::new(Legacy, "\x1b[27;9;99~", Some("super+c"), &["super+c"]),
    Row::new(
        Legacy,
        "\x1b[27;16;99~",
        Some("shift+ctrl+alt+super+c"),
        &["shift+ctrl+alt+super+c"],
    ),
    Row::new(Legacy, "\x1b[27;17;99~", None, &[]),
    Row {
        text: Some('c'),
        ..Row::new(Legacy, "\x1b[27;65;99~", Some("c"), &[])
    },
    Row {
        text: Some('c'),
        ..Row::new(Legacy, "\x1b[27;66;99~", Some("shift+c"), &[])
    },
    Row {
        text: Some('c'),
        ..Row::new(Legacy, "\x1b[27;193;99~", Some("c"), &[])
    },
    Row {
        text: Some('c'),
        ..Row::new(Legacy, "\x1b[27;194;99~", Some("shift+c"), &[])
    },
    Row::new(Legacy, "\x1b[27;0;122~", None, &[]),
    Row {
        text: Some('z'),
        ..Row::new(Legacy, "\x1b[27;1;122~", Some("z"), &[])
    },
    Row {
        text: Some('z'),
        ..Row::new(Legacy, "\x1b[27;2;122~", Some("shift+z"), &["shift+z"])
    },
    Row::new(Legacy, "\x1b[27;3;122~", Some("alt+z"), &["alt+z"]),
    Row::new(Legacy, "\x1b[27;5;122~", Some("ctrl+z"), &["ctrl+z"]),
    Row::new(
        Legacy,
        "\x1b[27;6;122~",
        Some("shift+ctrl+z"),
        &["shift+ctrl+z"],
    ),
    Row::new(
        Legacy,
        "\x1b[27;7;122~",
        Some("ctrl+alt+z"),
        &["ctrl+alt+z"],
    ),
    Row::new(Legacy, "\x1b[27;9;122~", Some("super+z"), &["super+z"]),
    Row::new(
        Legacy,
        "\x1b[27;16;122~",
        Some("shift+ctrl+alt+super+z"),
        &["shift+ctrl+alt+super+z"],
    ),
    Row::new(Legacy, "\x1b[27;17;122~", None, &[]),
    Row {
        text: Some('z'),
        ..Row::new(Legacy, "\x1b[27;65;122~", Some("z"), &[])
    },
    Row {
        text: Some('z'),
        ..Row::new(Legacy, "\x1b[27;66;122~", Some("shift+z"), &[])
    },
    Row {
        text: Some('z'),
        ..Row::new(Legacy, "\x1b[27;193;122~", Some("z"), &[])
    },
    Row {
        text: Some('z'),
        ..Row::new(Legacy, "\x1b[27;194;122~", Some("shift+z"), &[])
    },
    Row::new(Legacy, "\x1b[27;0;196~", None, &[]),
    Row {
        text: Some('\u{c4}'),
        ..Row::new(Legacy, "\x1b[27;1;196~", None, &[])
    },
    Row {
        text: Some('\u{c4}'),
        ..Row::new(Legacy, "\x1b[27;2;196~", None, &[])
    },
    Row::new(Legacy, "\x1b[27;3;196~", None, &[]),
    Row::new(Legacy, "\x1b[27;5;196~", None, &[]),
    Row::new(Legacy, "\x1b[27;6;196~", None, &[]),
    Row::new(Legacy, "\x1b[27;7;196~", None, &[]),
    Row::new(Legacy, "\x1b[27;9;196~", None, &[]),
    Row::new(Legacy, "\x1b[27;16;196~", None, &[]),
    Row::new(Legacy, "\x1b[27;17;196~", None, &[]),
    Row {
        text: Some('\u{c4}'),
        ..Row::new(Legacy, "\x1b[27;65;196~", None, &[])
    },
    Row {
        text: Some('\u{c4}'),
        ..Row::new(Legacy, "\x1b[27;66;196~", None, &[])
    },
    Row {
        text: Some('\u{c4}'),
        ..Row::new(Legacy, "\x1b[27;193;196~", None, &[])
    },
    Row {
        text: Some('\u{c4}'),
        ..Row::new(Legacy, "\x1b[27;194;196~", None, &[])
    },
    Row::new(Legacy, "\x1b[27;0;128512~", None, &[]),
    Row {
        text: Some('\u{1f600}'),
        ..Row::new(Legacy, "\x1b[27;1;128512~", None, &[])
    },
    Row {
        text: Some('\u{1f600}'),
        ..Row::new(Legacy, "\x1b[27;2;128512~", None, &[])
    },
    Row::new(Legacy, "\x1b[27;3;128512~", None, &[]),
    Row::new(Legacy, "\x1b[27;5;128512~", None, &[]),
    Row::new(Legacy, "\x1b[27;6;128512~", None, &[]),
    Row::new(Legacy, "\x1b[27;7;128512~", None, &[]),
    Row::new(Legacy, "\x1b[27;9;128512~", None, &[]),
    Row::new(Legacy, "\x1b[27;16;128512~", None, &[]),
    Row::new(Legacy, "\x1b[27;17;128512~", None, &[]),
    Row {
        text: Some('\u{1f600}'),
        ..Row::new(Legacy, "\x1b[27;65;128512~", None, &[])
    },
    Row {
        text: Some('\u{1f600}'),
        ..Row::new(Legacy, "\x1b[27;66;128512~", None, &[])
    },
    Row {
        text: Some('\u{1f600}'),
        ..Row::new(Legacy, "\x1b[27;193;128512~", None, &[])
    },
    Row {
        text: Some('\u{1f600}'),
        ..Row::new(Legacy, "\x1b[27;194;128512~", None, &[])
    },
];

/// Candidate identifiers tried against every row of [`MALFORMED`].
pub const MALFORMED_CAND: &[&str] = &[
    "escape",
    "esc",
    "enter",
    "return",
    "tab",
    "space",
    "backspace",
    "ctrl+backspace",
    "ctrl+h",
    "ctrl+space",
    "shift+enter",
    "alt+enter",
    "shift+tab",
    "up",
    "down",
    "left",
    "right",
    "home",
    "end",
    "insert",
    "delete",
    "pageUp",
    "pageDown",
    "clear",
    "f1",
    "f12",
    "a",
    "shift+a",
    "ctrl+a",
    "alt+a",
    "ctrl+alt+a",
];

/// Rows of the `malformed` probe.
pub const MALFORMED: &[Row] = &[
    Row::new(Legacy, "", None, &[]),
    Row::new(Legacy, "hello", None, &[]),
    Row::new(Legacy, "\u{e9}", None, &[]),
    Row::new(Legacy, "\u{1f600}", None, &[]),
    Row::new(Legacy, "\x1b[u", None, &[]),
    Row::new(Legacy, "\x1b[;1u", None, &[]),
    Row::new(Legacy, "\x1b[-1u", None, &[]),
    Row::new(Legacy, "\x1b[+99u", None, &[]),
    Row::new(Legacy, "\x1b[\u{ff19}\u{ff19}u", None, &[]),
    Row::new(Legacy, "\x1b[99.0u", None, &[]),
    Row::new(Legacy, "\x1b[99 u", None, &[]),
    Row::new(Legacy, "\x1b[99;u", None, &[]),
    Row::new(Legacy, "\x1b[99:;u", None, &[]),
    Row::new(Legacy, "\x1b[99:::97u", None, &[]),
    Row::new(Legacy, "\x1b[99::u", None, &[]),
    Row::new(Legacy, "\x1b[99;1:;u", None, &[]),
    Row::new(Legacy, "\x1b[99;1:2:3u", None, &[]),
    Row::new(Legacy, "\x1b[99;1U", None, &[]),
    Row::new(Legacy, "\x1b[99;5u\n", None, &[]),
    Row::new(Legacy, "\x1b[99;5u\r", None, &[]),
    Row::new(Legacy, "x\x1b[99u", None, &[]),
    Row::new(Legacy, "\x1b[99ux", None, &[]),
    Row::new(Legacy, "\x1b[99u\x1b[100u", None, &[]),
    Row::new(Legacy, "\x1b[27;5~", None, &[]),
    Row::new(Legacy, "\x1b[27;;99~", None, &[]),
    Row::new(Legacy, "\x1b[27;5;99", None, &[]),
    Row::new(Legacy, "\x1b[1;2E", None, &[]),
];

/// Candidate identifiers tried against every row of [`EVENTS`].
pub const EVENTS_CAND: &[&str] = &["c"];

/// Rows of the `events` probe.
pub const EVENTS: &[Row] = &[
    Row {
        kitty_text: Some('c'),
        text: Some('c'),
        ..Row::new(Both, "\x1b[99;1:1u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('c'),
        text: Some('c'),
        event: KeyEventType::Repeat,
        ..Row::new(Both, "\x1b[99;1:2u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('c'),
        text: Some('c'),
        event: KeyEventType::Release,
        ..Row::new(Both, "\x1b[99;1:3u", Some("c"), &["c"])
    },
    Row {
        kitty_text: Some('c'),
        text: Some('c'),
        ..Row::new(Both, "\x1b[99;1:4u", Some("c"), &["c"])
    },
    Row::new(Enhanced, "\x1b[200~x:3u\x1b[201~", None, &[]),
    Row::new(Enhanced, "text\x1b[200~\x1b[99;1:2u", None, &[]),
    Row::new(Enhanced, "\x1b[99;1:3u\x1b[200~", None, &[]),
];
