use super::color_supported;

/// Decides colour for a terminal flag and the given environment entries.
fn decide(stdout_is_terminal: bool, entries: &[(&str, &str)]) -> bool {
    color_supported(stdout_is_terminal, |name| {
        entries
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| (*value).to_owned())
    })
}

#[test]
fn forced_color_level_decides_before_the_terminal() {
    for (level, color) in [
        ("1", true),
        ("3", true),
        ("true", true),
        ("", true),
        ("0", false),
        ("false", false),
    ] {
        for terminal in [true, false] {
            assert_eq!(
                decide(terminal, &[("FORCE_COLOR", level)]),
                color,
                "FORCE_COLOR={level:?}, terminal {terminal}"
            );
        }
    }
}

#[test]
fn a_dumb_terminal_is_plain_unless_colour_is_forced() {
    assert!(!decide(true, &[("TERM", "dumb")]));
    assert!(decide(true, &[("TERM", "dumb"), ("FORCE_COLOR", "1")]));
}

#[test]
fn without_a_forced_level_only_a_terminal_takes_colour() {
    assert!(decide(true, &[]));
    assert!(decide(true, &[("TERM", "xterm-256color")]));
    assert!(!decide(false, &[]));
    assert!(!decide(false, &[("FORCE_COLOR", "unset-like")]));
}
