use super::color_supported;

/// Decides colour on Linux for a terminal flag and the given environment entries.
fn decide(stdout_is_terminal: bool, entries: &[(&str, &str)]) -> bool {
    decide_on("linux", stdout_is_terminal, entries)
}

/// Decides colour on the named platform for a terminal flag and environment entries.
fn decide_on(os: &str, stdout_is_terminal: bool, entries: &[(&str, &str)]) -> bool {
    color_supported(
        stdout_is_terminal,
        |name| {
            entries
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        },
        os,
    )
}

#[test]
fn forced_color_level_decides_before_the_terminal() {
    for (level, color) in [
        ("1", true),
        ("3", true),
        ("7", true),
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
fn a_level_that_is_not_a_number_still_lets_a_captured_stream_read_the_terminal() {
    assert!(!decide(false, &[("FORCE_COLOR", "unset-like")]));
    assert!(decide(
        false,
        &[("FORCE_COLOR", "unset-like"), ("TERM", "xterm")]
    ));
    assert!(!decide(false, &[("FORCE_COLOR", "-1"), ("CI", "true")]));
}

#[test]
fn a_dumb_terminal_is_plain_unless_colour_is_forced() {
    assert!(!decide(true, &[("TERM", "dumb")]));
    assert!(decide(true, &[("TERM", "dumb"), ("FORCE_COLOR", "1")]));
    assert!(!decide_on("windows", true, &[("TERM", "dumb")]));
}

#[test]
fn azure_pipelines_take_colour_before_the_terminal_check() {
    let azure = [("TF_BUILD", "True"), ("AGENT_NAME", "Hosted Agent")];
    assert!(decide(false, &azure));
    assert!(!decide(false, &azure[..1]));
    assert!(!decide(false, &[azure[0], azure[1], ("FORCE_COLOR", "0")]));
}

#[test]
fn a_captured_stream_without_forced_colour_is_plain() {
    assert!(!decide(false, &[]));
    assert!(!decide(false, &[("TERM", "xterm-256color")]));
    assert!(!decide(
        false,
        &[("CI", "true"), ("GITHUB_ACTIONS", "true")]
    ));
}

#[test]
fn windows_terminals_take_colour() {
    assert!(decide_on("windows", true, &[]));
    assert!(decide_on("windows", true, &[("CI", "true")]));
}

#[test]
fn continuous_integration_takes_colour_only_from_known_services() {
    let generic = ("CI", "true");
    assert!(!decide(true, &[generic, ("TERM", "xterm-256color")]));
    assert!(decide(true, &[generic, ("FORCE_COLOR", "1")]));
    for service in [
        "GITHUB_ACTIONS",
        "GITEA_ACTIONS",
        "CIRCLECI",
        "TRAVIS",
        "APPVEYOR",
        "GITLAB_CI",
        "BUILDKITE",
        "DRONE",
    ] {
        assert!(decide(true, &[generic, (service, "")]), "{service}");
        assert!(!decide(true, &[(service, "true")]), "{service} without CI");
    }
    assert!(decide(true, &[generic, ("CI_NAME", "codeship")]));
    assert!(!decide(true, &[generic, ("CI_NAME", "jenkins")]));
}

#[test]
fn teamcity_takes_colour_from_version_nine_point_one() {
    for (version, color) in [
        ("9.1.2", true),
        ("9.01.0", true),
        ("10.0", true),
        ("2024.12.1", true),
        ("9.0.5", false),
        ("9.1", false),
        ("8.1.1", false),
        ("10", false),
    ] {
        let entries = [("TEAMCITY_VERSION", version), ("TERM", "xterm-256color")];
        assert_eq!(decide(true, &entries), color, "TEAMCITY_VERSION={version}");
    }
}

#[test]
fn terminal_names_decide_colour() {
    for entries in [
        &[("COLORTERM", "truecolor")][..],
        &[("COLORTERM", "yes")],
        &[("TERM", "xterm-kitty")],
        &[("TERM", "xterm-ghostty")],
        &[("TERM", "wezterm")],
        &[("TERM_PROGRAM", "iTerm.app")],
        &[("TERM_PROGRAM", "Apple_Terminal")],
        &[("TERM", "konsole-256color")],
        &[("TERM", "putty-256")],
        &[("TERM", "screen")],
        &[("TERM", "XTERM")],
        &[("TERM", "vt100")],
        &[("TERM", "vt220")],
        &[("TERM", "rxvt-unicode")],
        &[("TERM", "st-color")],
        &[("TERM", "ansi")],
        &[("TERM", "cygwin")],
        &[("TERM", "linux")],
    ] {
        assert!(decide(true, entries), "{entries:?}");
    }
    for entries in [
        &[][..],
        &[("TERM", "")],
        &[("TERM", "foot")],
        &[("TERM", "WEZTERM")],
        &[("TERM_PROGRAM", "vscode")],
    ] {
        assert!(!decide(true, entries), "{entries:?}");
    }
}
