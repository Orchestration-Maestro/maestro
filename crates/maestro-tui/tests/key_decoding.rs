//! Terminal key decoding: identifiers, legacy and enhanced sequences, printable text and
//! protocol state.

use std::io;
use std::process::Command;
use std::sync::{Mutex, MutexGuard, PoisonError};

use maestro_tui::keys::decode_printable_key;
use maestro_tui::{
    Key, KeyEventType, decode_kitty_printable, is_key_release, is_key_repeat,
    is_kitty_protocol_active, matches_key, parse_key, set_kitty_protocol_active,
};

mod support {
    pub mod key_cases;
}
use support::key_cases::{
    CAND, ESCAPE, ESCAPE_CAND, EVENTS, EVENTS_CAND, KEYPAD, KEYPAD_CAND, LAYOUT, LAYOUT_CAND,
    LEGACY, LEGACY_CAND, MALFORMED, MALFORMED_CAND, MODIFIERS, MODIFIERS_CAND, MODIFY_OTHER_KEYS,
    MOK_CAND, Protocol, RAW, Row,
};

/// Serializes the tests that observe or change the process-wide protocol flag.
static PROTOCOL: Mutex<()> = Mutex::new(());

/// Variable that selects the scenario a probe child process runs.
const PROBE: &str = "MAESTRO_KEY_PROBE";

/// Variables that decide whether a raw backspace byte means ctrl+backspace.
const TERMINAL_VARIABLES: [&str; 4] = ["WT_SESSION", "SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"];

/// Holds the protocol flag for one test and restores its previous value afterwards.
struct ProtocolGuard {
    _lock: MutexGuard<'static, ()>,
    previous: bool,
    current: bool,
}

impl ProtocolGuard {
    /// Takes the flag exclusively and sets it to `active`.
    fn new(active: bool) -> Self {
        let lock = PROTOCOL.lock().unwrap_or_else(PoisonError::into_inner);
        let previous = is_kitty_protocol_active();
        set_kitty_protocol_active(active);
        Self {
            _lock: lock,
            previous,
            current: active,
        }
    }

    /// Changes the flag while the guard is held.
    fn set(&mut self, active: bool) {
        self.current = active;
        set_kitty_protocol_active(self.current);
    }
}

impl Drop for ProtocolGuard {
    fn drop(&mut self) {
        set_kitty_protocol_active(self.previous);
    }
}

/// Asserts that `data` matches `id`.
fn assert_key(data: &str, id: &str) {
    assert!(matches_key(data, id), "{data:?} should match {id}");
}

/// Asserts that `data` does not match `id`.
fn assert_not_key(data: &str, id: &str) {
    assert!(!matches_key(data, id), "{data:?} should not match {id}");
}

/// Asserts that `data` parses to `expected`.
fn assert_parse(data: &str, expected: Option<&str>) {
    assert_eq!(parse_key(data).as_deref(), expected, "parse {data:?}");
}

/// The protocol states a row is observed in.
fn states(protocol: Protocol) -> &'static [bool] {
    match protocol {
        Protocol::Legacy => &[false],
        Protocol::Enhanced => &[true],
        Protocol::Both => &[false, true],
    }
}

/// Asserts every observation of `row` on `data` in each protocol state the row names.
fn check_row_states(protocol: &mut ProtocolGuard, row: &Row, data: &str, candidates: &[&str]) {
    for &active in states(row.protocol) {
        protocol.set(active);
        check_row(row, data, candidates, active);
    }
}

/// Asserts every observation of each row in each protocol state it names.
fn check_rows(protocol: &mut ProtocolGuard, rows: &[Row], candidates: &[&str]) {
    for row in rows {
        check_row_states(protocol, row, row.data, candidates);
    }
}

/// Reads a `CSI code ; modifier u` input as its code field and modifier bits.
fn split_modifier_report(data: &str) -> Option<(&str, u32)> {
    let body = data.strip_prefix("\x1b[")?.strip_suffix('u')?;
    let (code, wire) = body.split_once(';')?;
    Some((code, wire.parse::<u32>().ok()?.checked_sub(1)?))
}

/// Asserts that the lock bits Caps Lock and Num Lock never change a row's observations.
///
/// Every row must be `CSI code ; modifier u`.
fn check_locked_rows(protocol: &mut ProtocolGuard, rows: &[Row], candidates: &[&str]) {
    for row in rows {
        let report = split_modifier_report(row.data);
        assert!(report.is_some(), "{:?} is not a modifier report", row.data);
        let Some((code, modifier)) = report else {
            continue;
        };
        for locks in [64, 128, 192] {
            let data = format!("\x1b[{code};{}u", (modifier | locks) + 1);
            check_row_states(protocol, row, &data, candidates);
        }
    }
}

/// Asserts every observation of `row` made on `data` against the current protocol state.
///
/// The row's parsed identifier is matched once, as a candidate of the table or, when the
/// table does not list it, as an extra one.
fn check_row(row: &Row, data: &str, candidates: &[&str], active: bool) {
    assert_eq!(
        parse_key(data).as_deref(),
        row.parsed,
        "parse {data:?} (enhanced: {active})"
    );
    let unlisted = row.parsed.filter(|id| !candidates.contains(id));
    for id in candidates.iter().copied().chain(unlisted) {
        assert_eq!(
            matches_key(data, id),
            row.matches.contains(&id),
            "match {data:?} against {id} (enhanced: {active})"
        );
    }
    assert_eq!(
        decode_kitty_printable(data),
        row.kitty_text,
        "kitty text {data:?}"
    );
    assert_eq!(decode_printable_key(data), row.text, "text {data:?}");
    assert_eq!(
        is_key_release(data),
        row.event == KeyEventType::Release,
        "release {data:?}"
    );
    assert_eq!(
        is_key_repeat(data),
        row.event == KeyEventType::Repeat,
        "repeat {data:?}"
    );
}

/// Runs `scenario` in a child process of this executable whose terminal variables are
/// exactly `variables`, and asserts that the child passes.
fn run_probe(scenario: &str, variables: &[(&str, &str)]) -> io::Result<()> {
    let mut child = Command::new(std::env::current_exe()?);
    child.args(["--exact", "environment_probe", "--test-threads=1"]);
    for name in TERMINAL_VARIABLES {
        child.env_remove(name);
    }
    let output = child
        .envs(variables.iter().copied())
        .env(PROBE, scenario)
        .output()?;
    assert!(
        output.status.success(),
        "probe {scenario} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

/// Asserts what the raw backspace byte means, with DEL and ctrl+h unaffected.
fn assert_raw_backspace(ctrl: bool) {
    let _protocol = ProtocolGuard::new(false);
    assert_key("\x7f", "backspace");
    assert_not_key("\x7f", "ctrl+backspace");
    assert_parse("\x7f", Some("backspace"));
    assert_eq!(matches_key("\x08", "backspace"), !ctrl);
    assert_eq!(matches_key("\x08", "ctrl+backspace"), ctrl);
    assert_parse(
        "\x08",
        Some(if ctrl { "ctrl+backspace" } else { "backspace" }),
    );
    assert_key("\x08", "ctrl+h");
}

/// The child half of [`run_probe`]; it does nothing unless a scenario is selected.
#[test]
fn environment_probe() {
    let Ok(scenario) = std::env::var(PROBE) else {
        return;
    };
    match scenario.as_str() {
        "raw_ascii" => {
            let mut protocol = ProtocolGuard::new(false);
            check_rows(&mut protocol, RAW, CAND);
        }
        "initial_state" => assert!(!is_kitty_protocol_active()),
        "plain_backspace" => assert_raw_backspace(false),
        "ctrl_backspace" => assert_raw_backspace(true),
        other => panic!("unknown probe scenario {other}"),
    }
}

/// Every string constant of [`Key`] with its literal identifier.
const KEY_CONSTANTS: [(&str, &str); 61] = [
    (Key::ESCAPE, "escape"),
    (Key::ESC, "esc"),
    (Key::ENTER, "enter"),
    (Key::RETURN, "return"),
    (Key::TAB, "tab"),
    (Key::SPACE, "space"),
    (Key::BACKSPACE, "backspace"),
    (Key::DELETE, "delete"),
    (Key::INSERT, "insert"),
    (Key::CLEAR, "clear"),
    (Key::HOME, "home"),
    (Key::END, "end"),
    (Key::PAGE_UP, "pageUp"),
    (Key::PAGE_DOWN, "pageDown"),
    (Key::UP, "up"),
    (Key::DOWN, "down"),
    (Key::LEFT, "left"),
    (Key::RIGHT, "right"),
    (Key::F1, "f1"),
    (Key::F2, "f2"),
    (Key::F3, "f3"),
    (Key::F4, "f4"),
    (Key::F5, "f5"),
    (Key::F6, "f6"),
    (Key::F7, "f7"),
    (Key::F8, "f8"),
    (Key::F9, "f9"),
    (Key::F10, "f10"),
    (Key::F11, "f11"),
    (Key::F12, "f12"),
    (Key::BACKTICK, "`"),
    (Key::HYPHEN, "-"),
    (Key::EQUALS, "="),
    (Key::LEFTBRACKET, "["),
    (Key::RIGHTBRACKET, "]"),
    (Key::BACKSLASH, "\\"),
    (Key::SEMICOLON, ";"),
    (Key::QUOTE, "'"),
    (Key::COMMA, ","),
    (Key::PERIOD, "."),
    (Key::SLASH, "/"),
    (Key::EXCLAMATION, "!"),
    (Key::AT, "@"),
    (Key::HASH, "#"),
    (Key::DOLLAR, "$"),
    (Key::PERCENT, "%"),
    (Key::CARET, "^"),
    (Key::AMPERSAND, "&"),
    (Key::ASTERISK, "*"),
    (Key::LEFTPAREN, "("),
    (Key::RIGHTPAREN, ")"),
    (Key::UNDERSCORE, "_"),
    (Key::PLUS, "+"),
    (Key::PIPE, "|"),
    (Key::TILDE, "~"),
    (Key::LEFTBRACE, "{"),
    (Key::RIGHTBRACE, "}"),
    (Key::COLON, ":"),
    (Key::LESSTHAN, "<"),
    (Key::GREATERTHAN, ">"),
    (Key::QUESTION, "?"),
];

/// A modifier constructor of [`Key`].
type Constructor = fn(&str) -> String;

/// Every modifier constructor of [`Key`] with the prefix it adds.
const KEY_CONSTRUCTORS: [(Constructor, &str); 18] = [
    (Key::ctrl, "ctrl+"),
    (Key::shift, "shift+"),
    (Key::alt, "alt+"),
    (Key::super_key, "super+"),
    (Key::ctrl_shift, "ctrl+shift+"),
    (Key::shift_ctrl, "shift+ctrl+"),
    (Key::ctrl_alt, "ctrl+alt+"),
    (Key::alt_ctrl, "alt+ctrl+"),
    (Key::shift_alt, "shift+alt+"),
    (Key::alt_shift, "alt+shift+"),
    (Key::ctrl_super, "ctrl+super+"),
    (Key::super_ctrl, "super+ctrl+"),
    (Key::shift_super, "shift+super+"),
    (Key::super_shift, "super+shift+"),
    (Key::alt_super, "alt+super+"),
    (Key::super_alt, "super+alt+"),
    (Key::ctrl_shift_alt, "ctrl+shift+alt+"),
    (Key::ctrl_shift_super, "ctrl+shift+super+"),
];

#[test]
fn key_helpers_preserve_constants_and_modifier_order() {
    for (constant, literal) in KEY_CONSTANTS {
        assert_eq!(constant, literal);
    }
    for (construct, prefix) in KEY_CONSTRUCTORS {
        for key in ["c", Key::ENTER, Key::SLASH, Key::PLUS] {
            assert_eq!(construct(key), format!("{prefix}{key}"));
        }
    }
}

#[test]
fn raw_ascii_and_legacy_ambiguities_keep_distinct_observations() -> io::Result<()> {
    run_probe("raw_ascii", &[])
}

#[test]
fn legacy_sequence_tables_cover_every_named_variant() {
    let mut protocol = ProtocolGuard::new(false);
    check_rows(&mut protocol, LEGACY, LEGACY_CAND);
}

#[test]
fn escape_prefixes_follow_protocol_mode_without_guessing() -> io::Result<()> {
    run_probe("initial_state", &[])?;
    let mut protocol = ProtocolGuard::new(false);
    check_rows(&mut protocol, ESCAPE, ESCAPE_CAND);
    protocol.set(true);
    assert!(is_kitty_protocol_active());
    protocol.set(false);
    assert!(!is_kitty_protocol_active());
    Ok(())
}

#[test]
fn maestro_keys_match_legacy_ctrl_symbol() {
    let _protocol = ProtocolGuard::new(false);
    assert_key("\x1f", "ctrl+_");
}

#[test]
fn maestro_keys_match_legacy_ctrl_alt_symbol() {
    let _protocol = ProtocolGuard::new(false);
    assert_key("\x1b\x1f", "ctrl+alt+_");
}

#[test]
fn escaped_direction_letters_are_alt_letters_only_while_the_protocol_is_inactive() {
    let mut protocol = ProtocolGuard::new(false);
    for (data, id) in [
        ("\x1bb", "alt+b"),
        ("\x1bf", "alt+f"),
        ("\x1bn", "alt+n"),
        ("\x1bp", "alt+p"),
    ] {
        protocol.set(false);
        assert_key(data, id);
        protocol.set(true);
        assert_not_key(data, id);
    }
}

#[test]
fn maestro_keys_ignore_alt_prefixed_sequences_when_kitty_active() {
    let _protocol = ProtocolGuard::new(true);
    for (data, id) in [
        ("\x1b\x03", "ctrl+alt+c"),
        ("\x1b1", "alt+1"),
        ("\x1by", "alt+y"),
        ("\x1bz", "alt+z"),
    ] {
        assert_not_key(data, id);
    }
}

#[test]
fn maestro_keys_treat_raw_0x08_as_plain_backspace_in_windows_terminal_over_ssh() -> io::Result<()> {
    run_probe(
        "plain_backspace",
        &[
            ("WT_SESSION", "test-session"),
            ("SSH_CONNECTION", "1 2 3 4"),
            ("SSH_CLIENT", "1 2 3"),
            ("SSH_TTY", "/dev/pts/1"),
        ],
    )
}

#[test]
fn maestro_keys_match_ctrl_c_when_pressing_ctrl_cyrillic_with_base_layout_key() {
    let _protocol = ProtocolGuard::new(true);
    assert_key("\x1b[1089::99;5u", "ctrl+c");
}

#[test]
fn maestro_keys_match_ctrl_d_when_pressing_ctrl_cyrillic_with_base_layout_key() {
    let _protocol = ProtocolGuard::new(true);
    assert_key("\x1b[1074::100;5u", "ctrl+d");
}

#[test]
fn maestro_keys_match_ctrl_z_when_pressing_ctrl_cyrillic_with_base_layout_key() {
    let _protocol = ProtocolGuard::new(true);
    assert_key("\x1b[1103::122;5u", "ctrl+z");
}

#[test]
fn maestro_keys_match_ctrl_shift_p_with_base_layout_key() {
    let _protocol = ProtocolGuard::new(true);
    assert_key("\x1b[1079::112;6u", "ctrl+shift+p");
}

#[test]
fn maestro_keys_still_match_direct_codepoint_when_no_base_layout_key() {
    let _protocol = ProtocolGuard::new(true);
    assert_key("\x1b[99;5u", "ctrl+c");
}

#[test]
fn maestro_keys_match_super_modified_kitty_bindings_including_combined_modifiers() {
    let _protocol = ProtocolGuard::new(true);
    assert_key("\x1b[107;9u", "super+k");
    assert_key("\x1b[13;9u", "super+enter");
    assert_key("\x1b[107;13u", &Key::ctrl_super("k"));
    assert_key("\x1b[107;14u", "ctrl+shift+super+k");
    assert_not_key("\x1b[107;13u", "super+k");
    assert_parse("\x1b[107;9u", Some("super+k"));
    assert_parse("\x1b[13;9u", Some("super+enter"));
    assert_parse("\x1b[107;13u", Some("ctrl+super+k"));
    assert_parse("\x1b[107;14u", Some("shift+ctrl+super+k"));
}

#[test]
fn maestro_keys_match_digit_bindings_via_kitty_csi_u() {
    let _protocol = ProtocolGuard::new(true);
    assert_key("\x1b[49u", "1");
    assert_key("\x1b[49;5u", "ctrl+1");
    assert_not_key("\x1b[49;5u", "ctrl+2");
    assert_parse("\x1b[49u", Some("1"));
    assert_parse("\x1b[49;5u", Some("ctrl+1"));
}

#[test]
fn maestro_keys_normalize_kitty_keypad_functional_keys_to_logical_digits_symbols_and_navigation() {
    let _protocol = ProtocolGuard::new(true);
    assert_key("\x1b[57400u", "1");
    assert_key("\x1b[57410u", "/");
    assert_key("\x1b[57417u", "left");
    assert_key("\x1b[57426u", "delete");
    for (code, id) in [
        (57399, "0"),
        (57409, "."),
        (57413, "+"),
        (57416, ","),
        (57417, "left"),
        (57418, "right"),
        (57419, "up"),
        (57420, "down"),
        (57421, "pageUp"),
        (57422, "pageDown"),
        (57423, "home"),
        (57424, "end"),
        (57425, "insert"),
        (57426, "delete"),
    ] {
        assert_parse(&format!("\x1b[{code}u"), Some(id));
    }
}

#[test]
fn maestro_keys_handle_shifted_key_in_format() {
    let _protocol = ProtocolGuard::new(true);
    assert_key("\x1b[99:67:99;2u", "shift+c");
}

#[test]
fn maestro_keys_handle_event_type_in_format() {
    let _protocol = ProtocolGuard::new(true);
    assert_key("\x1b[1089::99;5:3u", "ctrl+c");
}

#[test]
fn maestro_keys_handle_full_format_with_shifted_key_base_key_and_event_type() {
    let _protocol = ProtocolGuard::new(true);
    assert_key("\x1b[1089:1057:99;6:2u", "ctrl+shift+c");
}

#[test]
fn maestro_keys_prefer_codepoint_for_latin_letters_even_when_base_layout_differs() {
    let _protocol = ProtocolGuard::new(true);
    assert_key("\x1b[107::118;5u", "ctrl+k");
    assert_not_key("\x1b[107::118;5u", "ctrl+v");
}

#[test]
fn maestro_keys_prefer_codepoint_for_symbol_keys_even_when_base_layout_differs() {
    let _protocol = ProtocolGuard::new(true);
    assert_key("\x1b[47::91;5u", "ctrl+/");
    assert_not_key("\x1b[47::91;5u", "ctrl+[");
}

#[test]
fn maestro_keys_not_match_wrong_key_even_with_base_layout() {
    let _protocol = ProtocolGuard::new(true);
    assert_not_key("\x1b[1089::99;5u", "ctrl+d");
}

#[test]
fn maestro_keys_not_match_wrong_modifiers_even_with_base_layout() {
    let _protocol = ProtocolGuard::new(true);
    assert_not_key("\x1b[1089::99;5u", "ctrl+shift+c");
}

#[test]
fn maestro_keys_match_modifyotherkeys_ctrl_d() {
    let _protocol = ProtocolGuard::new(false);
    assert_key("\x1b[27;5;100~", "ctrl+d");
    assert_parse("\x1b[27;5;100~", Some("ctrl+d"));
}

/// Asserts that each modifyOtherKeys input matches and parses to its identifier.
fn assert_modify_other_keys(cases: &[(&str, &str)]) {
    let _protocol = ProtocolGuard::new(false);
    for (data, id) in cases {
        assert_key(data, id);
        assert_parse(data, Some(id));
    }
}

#[test]
fn maestro_keys_match_modifyotherkeys_backspace_variants() {
    assert_modify_other_keys(&[
        ("\x1b[27;1;127~", "backspace"),
        ("\x1b[27;5;127~", "ctrl+backspace"),
        ("\x1b[27;3;127~", "alt+backspace"),
    ]);
}

#[test]
fn maestro_keys_match_ctrl_alt_letter_via_csi_u_when_kitty_inactive() {
    let _protocol = ProtocolGuard::new(false);
    assert_key("\x1b[104;7u", "ctrl+alt+h");
    assert_parse("\x1b[104;7u", Some("ctrl+alt+h"));
}

#[test]
fn maestro_keys_match_ctrl_alt_letter_via_modifyotherkeys() {
    assert_modify_other_keys(&[("\x1b[27;7;104~", "ctrl+alt+h")]);
}

#[test]
fn maestro_keys_decode_kitty_keypad_functional_keys_to_printable_characters() {
    for (code, text) in [
        (57399, '0'),
        (57400, '1'),
        (57409, '.'),
        (57410, '/'),
        (57411, '*'),
        (57412, '-'),
        (57413, '+'),
        (57415, '='),
        (57416, ','),
    ] {
        assert_eq!(decode_kitty_printable(&format!("\x1b[{code}u")), Some(text));
    }
}

#[test]
fn maestro_keys_return_latin_key_name_when_base_layout_key_is_present() {
    let _protocol = ProtocolGuard::new(true);
    assert_parse("\x1b[1089::99;5u", Some("ctrl+c"));
}

#[test]
fn maestro_keys_prefer_codepoint_for_latin_letters_when_base_layout_differs() {
    let _protocol = ProtocolGuard::new(true);
    assert_parse("\x1b[107::118;5u", Some("ctrl+k"));
}

#[test]
fn maestro_keys_prefer_codepoint_for_symbol_keys_when_base_layout_differs() {
    let _protocol = ProtocolGuard::new(true);
    assert_parse("\x1b[47::91;5u", Some("ctrl+/"));
}

#[test]
fn maestro_keys_return_key_name_from_codepoint_when_no_base_layout() {
    let _protocol = ProtocolGuard::new(true);
    assert_parse("\x1b[99;5u", Some("ctrl+c"));
}

#[test]
fn maestro_keys_parse_shifted_uppercase_csi_u_letters_as_shift_letter() {
    let _protocol = ProtocolGuard::new(true);
    assert_key("\x1b[69;2u", "shift+e");
    assert_parse("\x1b[69;2u", Some("shift+e"));
}

#[test]
fn maestro_keys_ignore_kitty_csi_u_with_unsupported_modifiers() {
    let _protocol = ProtocolGuard::new(true);
    assert_parse("\x1b[99;17u", None);
}

#[test]
fn enhanced_modifiers_mask_locks_and_reject_other_bits() {
    let mut protocol = ProtocolGuard::new(false);
    check_rows(&mut protocol, MODIFIERS, MODIFIERS_CAND);
    check_locked_rows(&mut protocol, MODIFIERS, MODIFIERS_CAND);
}

#[test]
fn alternate_layout_fields_keep_logical_key_authority() {
    let mut protocol = ProtocolGuard::new(false);
    check_rows(&mut protocol, LAYOUT, LAYOUT_CAND);
}

#[test]
fn keypad_table_normalizes_each_digit_symbol_and_navigation() {
    let mut protocol = ProtocolGuard::new(false);
    check_rows(&mut protocol, KEYPAD, KEYPAD_CAND);
}

#[test]
fn modify_other_keys_keeps_its_own_matching_rules() {
    let mut protocol = ProtocolGuard::new(false);
    check_rows(&mut protocol, MODIFY_OTHER_KEYS, MOK_CAND);
}

#[test]
fn malformed_sequences_do_not_become_keys_or_printable_text() {
    let mut protocol = ProtocolGuard::new(false);
    check_rows(&mut protocol, MALFORMED, MALFORMED_CAND);
}

/// Key names whose identifiers an enhanced navigation report must not be mistaken for.
const NAVIGATION_NAMES: [&str; 13] = [
    "up", "down", "left", "right", "home", "end", "insert", "delete", "pageUp", "pageDown",
    "clear", "f1", "f12",
];

/// Modifier prefixes tried against navigation reports.
const NAVIGATION_PREFIXES: [&str; 6] = [
    "",
    "shift+",
    "alt+",
    "ctrl+",
    "super+",
    "shift+ctrl+alt+super+",
];

/// Asserts the identity, typed text and phase of one navigation report.
fn assert_report(data: &str, parsed: Option<&str>, event: KeyEventType) {
    assert_parse(data, parsed);
    for name in NAVIGATION_NAMES {
        for prefix in NAVIGATION_PREFIXES {
            let id = format!("{prefix}{name}");
            assert_eq!(
                matches_key(data, &id),
                parsed == Some(id.as_str()),
                "match {data:?} against {id}"
            );
        }
    }
    assert_eq!(decode_kitty_printable(data), None, "kitty text {data:?}");
    assert_eq!(decode_printable_key(data), None, "text {data:?}");
    assert_eq!(
        is_key_release(data),
        event == KeyEventType::Release,
        "release {data:?}"
    );
    assert_eq!(
        is_key_repeat(data),
        event == KeyEventType::Repeat,
        "repeat {data:?}"
    );
}

/// Wire modifier fields and the identifier prefix each stands for, lock bits included.
const NAVIGATION_MODIFIERS: [(u32, &str); 8] = [
    (1, ""),
    (2, "shift+"),
    (3, "alt+"),
    (5, "ctrl+"),
    (9, "super+"),
    (16, "shift+ctrl+alt+super+"),
    (65, ""),
    (193, ""),
];

/// Event fields and the phase each reports.
const NAVIGATION_EVENTS: [(&str, KeyEventType); 6] = [
    ("", KeyEventType::Press),
    (":0", KeyEventType::Press),
    (":1", KeyEventType::Press),
    (":2", KeyEventType::Repeat),
    (":3", KeyEventType::Release),
    (":4", KeyEventType::Press),
];

/// Final bytes of arrow and home/end reports with their key names.
const FINAL_KEYS: [(char, &str); 6] = [
    ('A', "up"),
    ('B', "down"),
    ('C', "right"),
    ('D', "left"),
    ('H', "home"),
    ('F', "end"),
];

/// Tilde key numbers that name a key.
const NAMED_TILDES: [(u32, &str); 6] = [
    (2, "insert"),
    (3, "delete"),
    (5, "pageUp"),
    (6, "pageDown"),
    (7, "home"),
    (8, "end"),
];

/// Tilde key numbers that name no key.
const UNNAMED_TILDES: [u32; 7] = [1, 4, 9, 11, 24, 200, 201];

/// Checks every report that carries a modifier field with one event field.
fn check_modified_reports(wire: u32, prefix: &str, suffix: &str, event: KeyEventType) {
    for (end, name) in FINAL_KEYS {
        let id = format!("{prefix}{name}");
        assert_report(&format!("\x1b[1;{wire}{suffix}{end}"), Some(&id), event);
    }
    for (code, name) in NAMED_TILDES {
        let id = format!("{prefix}{name}");
        assert_report(&format!("\x1b[{code};{wire}{suffix}~"), Some(&id), event);
    }
    for code in UNNAMED_TILDES {
        assert_report(&format!("\x1b[{code};{wire}{suffix}~"), None, event);
    }
}

/// Checks tilde reports with an event field but no modifier field.
fn check_unmodified_reports(suffix: &str, event: KeyEventType) {
    for (code, name) in NAMED_TILDES {
        assert_report(&format!("\x1b[{code}{suffix}~"), Some(name), event);
    }
    for code in UNNAMED_TILDES {
        assert_report(&format!("\x1b[{code}{suffix}~"), None, event);
    }
}

#[test]
fn enhanced_navigation_forms_keep_event_and_modifier_fields() {
    let mut protocol = ProtocolGuard::new(false);
    for active in [false, true] {
        protocol.set(active);
        for (wire, prefix) in NAVIGATION_MODIFIERS {
            for (suffix, event) in NAVIGATION_EVENTS {
                check_modified_reports(wire, prefix, suffix, event);
            }
        }
        for (suffix, event) in NAVIGATION_EVENTS.into_iter().skip(1) {
            check_unmodified_reports(suffix, event);
        }
        for code in [9, 200, 201] {
            assert_report(&format!("\x1b[{code}~"), None, KeyEventType::Press);
        }
    }
    let colon_events = [
        ("\x1b[99:67:99:3u", KeyEventType::Release),
        ("\x1b[99::99:2u", KeyEventType::Repeat),
        ("\x1b[99:67:99:4u", KeyEventType::Press),
    ];
    for (data, event) in colon_events {
        assert_parse(data, Some("c"));
        assert_key(data, "c");
        assert_eq!(
            is_key_release(data),
            event == KeyEventType::Release,
            "release {data:?}"
        );
        assert_eq!(
            is_key_repeat(data),
            event == KeyEventType::Repeat,
            "repeat {data:?}"
        );
        assert_eq!(decode_printable_key(data), Some('c'));
    }
}

#[test]
fn identifier_aliases_case_and_modifier_order_match() {
    let _protocol = ProtocolGuard::new(false);
    let inputs = ["\x1b", "\r", "\x1b[5~", "\x1b[99;6u", "\x1b[99;13u", "\x03"];
    let identifiers = [
        "ESCAPE",
        "RETURN",
        "pageup",
        "PAGEUP",
        "ctrl+shift+c",
        "super+ctrl+c",
        "ctrl+ctrl+c",
        "hyper+ctrl+c",
        "",
    ];
    let accepted = [
        ("\x1b", "ESCAPE"),
        ("\r", "RETURN"),
        ("\x1b[5~", "pageup"),
        ("\x1b[5~", "PAGEUP"),
        ("\x1b[99;6u", "ctrl+shift+c"),
        ("\x1b[99;13u", "super+ctrl+c"),
        ("\x03", "ctrl+ctrl+c"),
        ("\x03", "hyper+ctrl+c"),
    ];
    for data in inputs {
        for id in identifiers {
            assert_eq!(
                matches_key(data, id),
                accepted.contains(&(data, id)),
                "match {data:?} against {id:?}"
            );
        }
    }
}

#[test]
fn windows_backspace_uses_each_environment_truth_value() -> io::Result<()> {
    for session in [None, Some(""), Some("session")] {
        let session_ctrl = session == Some("session");
        let scenario = |ctrl: bool| {
            if ctrl {
                "ctrl_backspace"
            } else {
                "plain_backspace"
            }
        };
        let wt: Vec<(&str, &str)> = session
            .map(|value| ("WT_SESSION", value))
            .into_iter()
            .collect();
        if !wt.is_empty() {
            run_probe(scenario(session_ctrl), &wt)?;
        }
        for name in ["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"] {
            for (value, remote) in [("", false), ("active", true)] {
                let mut variables = wt.clone();
                variables.push((name, value));
                run_probe(scenario(session_ctrl && !remote), &variables)?;
            }
        }
    }
    Ok(())
}

#[test]
fn release_and_repeat_are_independent_of_previous_calls() {
    let mut protocol = ProtocolGuard::new(false);
    check_rows(&mut protocol, EVENTS, EVENTS_CAND);
    let release = "\x1b[100;1:3u";
    let repeat = "\x1b[100;1:2u";
    for active in [false, true] {
        protocol.set(active);
        for between in ["\x1b[99;1:2u", "\x1b[1;5A", "x", "\x1b[200~", ""] {
            // Only the call matters here: it must leave both predicates unchanged.
            let _ = parse_key(between);
            assert!(is_key_release(release) && !is_key_repeat(release));
            assert!(is_key_repeat(repeat) && !is_key_release(repeat));
        }
    }
}

#[test]
fn plus_identifiers_match_literal_plus_in_every_supported_encoding() {
    let mut protocol = ProtocolGuard::new(false);
    let cases = [
        ("+", "+", true),
        ("\x1b[43u", "+", true),
        ("\x1b[43;2u", "shift++", true),
        ("\x1b[43;5u", "ctrl++", true),
        ("\x1b[43;9u", "super++", true),
        ("\x1b[27;5;43~", "ctrl++", true),
        ("\x1b[57413u", "+", true),
        ("\x1b[57413u", "ctrl++", false),
        ("\x1b[43;5u", "ctrl+", false),
        ("\x1b[27;1;43~", "+", false),
        ("+", "ctrl++", false),
        ("\x1b+", "alt++", false),
    ];
    for active in [false, true] {
        protocol.set(active);
        for (data, id, expected) in cases {
            assert_eq!(
                matches_key(data, id),
                expected,
                "match {data:?} against {id}"
            );
        }
    }
    assert_parse("+", Some("+"));
    assert_parse("\x1b[43;5u", Some("ctrl++"));
}

#[test]
fn digit_layout_alternatives_never_override_logical_digits() {
    let _protocol = ProtocolGuard::new(false);
    for (code, digit) in [(48, '0'), (49, '1'), (57, '9'), (57400, '1')] {
        for (wire, prefix) in [(1, ""), (2, "shift+"), (5, "ctrl+")] {
            let data = format!("\x1b[{code}::50;{wire}u");
            let id = format!("{prefix}{digit}");
            assert_parse(&data, Some(&id));
            assert_key(&data, &id);
            assert_not_key(&data, &format!("{prefix}2"));
        }
    }
}

#[test]
fn numeric_fields_and_unicode_scalars_never_wrap_or_alias() {
    let _protocol = ProtocolGuard::new(false);
    let huge_code = format!("\x1b[{}u", "9".repeat(90));
    let cases = [
        ("\x1b[65583u", Some('\u{1002f}')),
        ("\x1b[1114159u", None),
        ("\x1b[55296u", None),
        ("\x1b[57343u", None),
        ("\x1b[1114112u", None),
        (huge_code.as_str(), None),
        ("\x1b[99;4294967301u", None),
        ("\x1b[99;4294967297u", None),
        ("\x1b[27;4294967297;99~", None),
        ("\x1b[27;1;55296~", None),
        ("\x1b[97:55296;2u", None),
        ("\x1b[97::55296;1u", None),
        ("\x1b[97:4294967393;2u", None),
        ("\x1b[97::4294967393;1u", None),
        ("\x1b[4294967298;5~", None),
    ];
    for (data, text) in cases {
        assert_parse(data, None);
        assert_eq!(decode_kitty_printable(data), text, "kitty text {data:?}");
        if text.is_some() || !data.starts_with("\x1b[27;") {
            assert_eq!(decode_printable_key(data), text, "text {data:?}");
        }
        for id in ["/", "ctrl+c", "c"] {
            assert_not_key(data, id);
        }
    }
}

#[test]
fn event_predicates_require_complete_events_not_text_fragments() {
    let fragments = [
        "x:3u",
        "x:2F",
        "\x1b[99:3u",
        "\x1b[99:2u",
        "\x1b[99;1:3ux",
        "x\x1b[99;1:3u",
        "\x1b[99;1:2u\x1b[99;1:3u",
        "\x1b[99;:3u",
        "\x1b[99;1:33u",
        "\x1b[99;1:22u",
    ];
    for data in fragments {
        assert!(!is_key_release(data) && !is_key_repeat(data), "{data:?}");
    }
    for (data, release, repeat) in [
        ("\x1b[99;0:3u", true, false),
        ("\x1b[55296;1:2u", false, true),
        ("\x1b[99;1:99999999999999999999u", false, false),
    ] {
        assert_eq!(is_key_release(data), release, "release {data:?}");
        assert_eq!(is_key_repeat(data), repeat, "repeat {data:?}");
    }
}

#[test]
fn printable_decoding_excludes_controls_and_functional_keycodes() {
    let csi_u = [
        (31, false),
        (32, true),
        (126, true),
        (127, false),
        (128, false),
        (159, false),
        (160, true),
        (0xd7ff, true),
        (0xe000, false),
        (0xe020, false),
        (0xe04e, false),
        (0xe0ff, false),
        (0xe100, false),
        (0xf8ff, false),
        (0xf900, true),
        (0xffff, true),
        (0x1_0000, true),
        (0x10_ffff, true),
    ];
    for (code, printable) in csi_u {
        let data = format!("\x1b[{code}u");
        let text = char::from_u32(code).filter(|_| printable);
        assert_eq!(decode_kitty_printable(&data), text, "{data:?}");
        assert_eq!(decode_printable_key(&data), text, "{data:?}");
    }
    let modify_other_keys = [
        (126, true),
        (127, false),
        (128, false),
        (159, false),
        (160, true),
        (0xd7ff, true),
        (0xd800, false),
        (0xdfff, false),
        (0xe000, true),
        (0xe020, true),
        (0xe04e, true),
        (0xe0ff, true),
        (0xe100, true),
        (0xf8ff, true),
        (0xf900, true),
        (0xffff, true),
        (0x1_0000, true),
        (0x10_ffff, true),
        (0x11_0000, false),
    ];
    for (code, printable) in modify_other_keys {
        let data = format!("\x1b[27;1;{code}~");
        let text = char::from_u32(code).filter(|_| printable);
        assert_eq!(decode_printable_key(&data), text, "{data:?}");
    }
    for code in [57414, 57417, 57426] {
        assert_eq!(
            decode_kitty_printable(&format!("\x1b[{code}u")),
            None,
            "{code}"
        );
    }
}

#[test]
fn decimal_fields_accept_leading_zeros_without_engine_wrap() {
    let _protocol = ProtocolGuard::new(false);
    let long_event = format!("\x1b[00099;00005:{}u", "9".repeat(400));
    let cases = [
        ("\x1b[00099;00005u", Some("ctrl+c"), KeyEventType::Press),
        (
            "\x1b[00099::00099;00005:00002u",
            Some("ctrl+c"),
            KeyEventType::Repeat,
        ),
        (
            "\x1b[00099;00005:00003u",
            Some("ctrl+c"),
            KeyEventType::Release,
        ),
        (
            "\x1b[00099;00005:00000u",
            Some("ctrl+c"),
            KeyEventType::Press,
        ),
        (long_event.as_str(), Some("ctrl+c"), KeyEventType::Press),
        ("\x1b[27;00005;00099~", Some("ctrl+c"), KeyEventType::Press),
        ("\x1b[99;0u", None, KeyEventType::Press),
        ("\x1b[99;1u\u{85}", None, KeyEventType::Press),
        ("\x1b[99;1u\u{feff}", None, KeyEventType::Press),
    ];
    for (data, parsed, event) in cases {
        assert_parse(data, parsed);
        assert_eq!(
            matches_key(data, "ctrl+c"),
            parsed.is_some(),
            "match {data:?}"
        );
        assert_not_key(data, "c");
        assert_eq!(
            is_key_release(data),
            event == KeyEventType::Release,
            "release {data:?}"
        );
        assert_eq!(
            is_key_repeat(data),
            event == KeyEventType::Repeat,
            "repeat {data:?}"
        );
        assert_eq!(decode_kitty_printable(data), None, "kitty text {data:?}");
        assert_eq!(decode_printable_key(data), None, "text {data:?}");
    }
}
