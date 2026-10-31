//! Workspace key-combination grammar over shared key codes.
#![cfg(test)]

use maestro_daemon::{
    input::TerminalKey,
    key_combo::{
        format_key_combo, normalize_key_combo, parse_key_combo, terminal_key_matches_combo,
    },
};
use maestro_protocol::frames::{ClientKeyCode as KeyCode, KeyModifiers};

#[test]
fn key_combo_accepts_unicode_char_combo() {
    assert_eq!(
        parse_key_combo("ö"),
        Some((KeyCode::Char('ö'), KeyModifiers::NONE))
    );
    assert_eq!(
        parse_key_combo("alt+é"),
        Some((KeyCode::Char('é'), KeyModifiers::ALT))
    );
}

#[test]
fn key_combo_accepts_shift_tab_as_backtab() {
    assert_eq!(
        parse_key_combo("shift+tab"),
        Some((KeyCode::BackTab, KeyModifiers::NONE))
    );
}

#[test]
fn key_combo_accepts_named_punctuation() {
    for (name, character) in [("minus", '-'), ("comma", ','), ("ampersand", '&')] {
        assert_eq!(
            parse_key_combo(name),
            Some((KeyCode::Char(character), KeyModifiers::NONE))
        );
    }
}

#[test]
fn key_combinations_preserve_named_aliases_and_modifier_meaning() {
    named_key_aliases();
    modifier_aliases();
    canonical_formatting();
    shifted_key_matching();
}

/// Key names retain their workspace spelling rather than the transport vocabulary.
fn named_key_aliases() {
    for (name, code) in [
        ("space", KeyCode::Char(' ')),
        ("enter", KeyCode::Enter),
        ("return", KeyCode::Enter),
        ("esc", KeyCode::Esc),
        ("escape", KeyCode::Esc),
        ("bs", KeyCode::Backspace),
        ("backspace", KeyCode::Backspace),
        ("left", KeyCode::Left),
        ("right", KeyCode::Right),
        ("up", KeyCode::Up),
        ("down", KeyCode::Down),
        ("period", KeyCode::Char('.')),
        ("slash", KeyCode::Char('/')),
        ("backslash", KeyCode::Char('\\')),
        ("quote", KeyCode::Char('\'')),
        ("double_quote", KeyCode::Char('"')),
        ("double-quote", KeyCode::Char('"')),
        ("semicolon", KeyCode::Char(';')),
        ("colon", KeyCode::Char(':')),
        ("percent", KeyCode::Char('%')),
        ("backtick", KeyCode::Char('`')),
        ("plus", KeyCode::Char('+')),
        ("F12", KeyCode::F(12)),
    ] {
        assert_eq!(
            parse_key_combo(name),
            Some((code, KeyModifiers::NONE)),
            "{name}"
        );
    }
}

/// Modifier aliases preserve combined flags and reject malformed combinations.
fn modifier_aliases() {
    for (alias, modifier) in [
        ("ctrl", KeyModifiers::CONTROL),
        ("control", KeyModifiers::CONTROL),
        ("option", KeyModifiers::ALT),
        ("meta", KeyModifiers::ALT),
        ("cmd", KeyModifiers::SUPER),
        ("command", KeyModifiers::SUPER),
        ("super", KeyModifiers::SUPER),
        ("hyper", KeyModifiers::HYPER),
    ] {
        assert_eq!(
            parse_key_combo(&format!("{alias}+v")),
            Some((KeyCode::Char('v'), modifier))
        );
    }
    assert_eq!(
        parse_key_combo(" Ctrl + V "),
        Some((
            KeyCode::Char('v'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT
        ))
    );
    for invalid in [
        "",
        "ctrl",
        "+",
        "ctrl++v",
        "v+w",
        "f256",
        "unknown",
        "ctrl+shift",
    ] {
        assert!(parse_key_combo(invalid).is_none(), "{invalid}");
    }
}

/// Formatting removes redundant shift and retains canonical modifier order.
fn canonical_formatting() {
    assert_eq!(
        normalize_key_combo((KeyCode::BackTab, KeyModifiers::ALT | KeyModifiers::SHIFT)),
        (KeyCode::BackTab, KeyModifiers::ALT)
    );
    assert_eq!(
        format_key_combo((
            KeyCode::BackTab,
            KeyModifiers::CONTROL | KeyModifiers::SHIFT
        )),
        "ctrl+shift+tab"
    );
    assert_eq!(
        format_key_combo((KeyCode::Char(' '), KeyModifiers::ALT)),
        "alt+space"
    );
    assert_eq!(
        format_key_combo((KeyCode::F(12), KeyModifiers::META)),
        "meta+f12"
    );
}

/// Shifted code points match only the requested code and modifier combination.
fn shifted_key_matching() {
    let shifted = TerminalKey::new(
        KeyCode::Char('1'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    )
    .with_shifted_codepoint(u32::from('!'));
    assert!(terminal_key_matches_combo(
        &shifted,
        (KeyCode::Char('!'), KeyModifiers::CONTROL)
    ));
    assert!(!terminal_key_matches_combo(
        &shifted,
        (KeyCode::Char('@'), KeyModifiers::CONTROL)
    ));
    assert!(!terminal_key_matches_combo(
        &shifted,
        (KeyCode::Char('!'), KeyModifiers::ALT)
    ));
    assert!(terminal_key_matches_combo(
        &TerminalKey::new(KeyCode::Char('V'), KeyModifiers::CONTROL),
        (
            KeyCode::Char('v'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT
        )
    ));
    assert!(terminal_key_matches_combo(
        &TerminalKey::new(KeyCode::Char('?'), KeyModifiers::SHIFT),
        (KeyCode::Char('?'), KeyModifiers::NONE)
    ));
    assert!(terminal_key_matches_combo(
        &TerminalKey::new(KeyCode::Tab, KeyModifiers::SHIFT),
        (KeyCode::BackTab, KeyModifiers::NONE)
    ));
    assert!(!terminal_key_matches_combo(
        &TerminalKey::new(KeyCode::Char('ö'), KeyModifiers::NONE),
        (KeyCode::Char('Ö'), KeyModifiers::NONE)
    ));
}

#[test]
fn key_combo_accepts_simple_char_combo() {
    assert_eq!(
        parse_key_combo("v"),
        Some((KeyCode::Char('v'), KeyModifiers::NONE))
    );
}

/// One spelling, a key report that names it for the chat keybindings, and the workspace combination.
type Spelling = (&'static str, &'static str, (KeyCode, u8));

#[test]
fn one_key_grammar_for_both_frontends() {
    let spellings: [Spelling; 5] = [
        (
            "ctrl++",
            "\x1b[43;5u",
            (KeyCode::Char('+'), KeyModifiers::CONTROL),
        ),
        ("+", "+", (KeyCode::Char('+'), KeyModifiers::NONE)),
        ("ö", "ö", (KeyCode::Char('ö'), KeyModifiers::NONE)),
        (
            "alt+é",
            "\x1b[233;3u",
            (KeyCode::Char('é'), KeyModifiers::ALT),
        ),
        (
            "shift+tab",
            "\x1b[Z",
            (KeyCode::BackTab, KeyModifiers::NONE),
        ),
    ];
    for (spelling, report, combo) in spellings {
        assert_eq!(
            parse_key_combo(spelling),
            Some(combo),
            "workspace {spelling}"
        );
        assert!(
            maestro_tui::matches_key(report, spelling),
            "chat {spelling}"
        );
    }

    for rejected in ["ctrl+", "ctrl+a+b"] {
        assert_eq!(parse_key_combo(rejected), None, "workspace {rejected}");
        for report in ["\x01", "\x02", "\x1b[97;5u", "\x1b[98;5u", "a", "b"] {
            assert!(
                !maestro_tui::matches_key(report, rejected),
                "chat {rejected} {report:?}"
            );
        }
    }
}

#[test]
fn hyper_binding_requires_hyper() {
    let hyper_k = parse_key_combo("hyper+k").expect("hyper+k parses");
    assert_eq!(hyper_k, (KeyCode::Char('k'), KeyModifiers::HYPER));
    assert!(terminal_key_matches_combo(
        &TerminalKey::new(KeyCode::Char('k'), KeyModifiers::HYPER),
        hyper_k.clone()
    ));
    assert!(!terminal_key_matches_combo(
        &TerminalKey::new(KeyCode::Char('k'), KeyModifiers::NONE),
        hyper_k
    ));

    assert!(maestro_tui::matches_key("\x1b[107;17u", "hyper+k"));
    for report in ["k", "\x1b[107;5u", "\x1b[107;9u", "\x1b[107;1u"] {
        assert!(
            !maestro_tui::matches_key(report, "hyper+k"),
            "chat hyper+k {report:?}"
        );
    }
}

#[test]
fn unknown_modifier_word_is_rejected() {
    for word in ["ctlr", "contrl", "windows", "shfit", "a"] {
        let spelling = format!("{word}+k");
        assert_eq!(parse_key_combo(&spelling), None, "workspace {spelling}");
        for report in ["k", "\x0b", "\x1b[107;5u"] {
            assert!(
                !maestro_tui::matches_key(report, &spelling),
                "chat {spelling} {report:?}"
            );
        }
    }
}
