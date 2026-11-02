//! Launch edits observed by actual shell children.
use super::*;
use maestro_process::pty::{PtyCommand, PtyEnvironment};
use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    path::Path,
    process::Command,
};

/// Execute the environment produced by the pane launch policy.
fn capture(extra: Vec<(String, String)>, script: &str) -> String {
    let mut description = maestro_process::shell::pane_custom_command_pty_builder(script);
    description.environment = PtyEnvironment::Replace(vec![
        ("TERM".into(), "outer-terminal".into()),
        ("COLORTERM".into(), "outer-color".into()),
        ("CODEX_THREAD_ID".into(), "outer-session".into()),
    ]);
    PaneLaunchEnv::from_extra(extra).apply(&mut description, Path::new("/tmp/maestro.sock"), None);
    let mut command = Command::new(description.program);
    command.args(description.args).env_clear();
    let PtyEnvironment::Replace(values) = description.environment else {
        panic!("replacement policy must remain explicit")
    };
    command.envs(values);
    let output = command.output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn outer_agent_thread_is_not_inherited_by_panes() {
    assert_eq!(
        capture(vec![], "printf '%s' \"${CODEX_THREAD_ID-unset}\""),
        "unset"
    );
}

#[test]
fn panes_advertise_their_own_terminal_capabilities() {
    assert_eq!(
        capture(vec![], "printf '%s\\n%s\\n' \"$TERM\" \"$COLORTERM\""),
        "xterm-256color\ntruecolor\n"
    );
}

#[test]
fn explicit_environment_can_override_terminal_capabilities() {
    assert_eq!(
        capture(
            vec![
                ("TERM".into(), "vt100".into()),
                ("COLORTERM".into(), "24bit".into())
            ],
            "printf '%s\\n%s\\n' \"$TERM\" \"$COLORTERM\""
        ),
        "vt100\n24bit\n"
    );
}

/// Read effective ordered edits for a key.
fn value(command: &PtyCommand, key: &str) -> Option<OsString> {
    let PtyEnvironment::Inherit(edits) = &command.environment else {
        panic!("expected inheritance")
    };
    edits
        .iter()
        .rev()
        .find(|(name, _)| name == key)
        .and_then(|(_, value)| value.clone())
}

#[test]
fn managed_identity_and_base_paths_override_user_pairs() {
    let mut command = maestro_process::shell::pane_custom_command_pty_builder("");
    let launch = PaneLaunchEnv::from_extra(vec![
        ("MAESTRO_ENV".into(), "0".into()),
        ("MAESTRO_PANE_ID".into(), "outer".into()),
        ("CUSTOM".into(), "yes".into()),
    ])
    .with_identity("workspace".into(), "tab".into(), "pane".into());
    launch.apply(
        &mut command,
        Path::new("/tmp/maestro.sock"),
        Some(Path::new("/bin/maestro")),
    );
    for (key, expected) in [
        ("MAESTRO_ENV", "1"),
        ("MAESTRO_SOCKET_PATH", "/tmp/maestro.sock"),
        ("MAESTRO_BIN_PATH", "/bin/maestro"),
        ("MAESTRO_WORKSPACE_ID", "workspace"),
        ("MAESTRO_TAB_ID", "tab"),
        ("MAESTRO_PANE_ID", "pane"),
        ("CUSTOM", "yes"),
    ] {
        assert_eq!(value(&command, key), Some(expected.into()));
        assert!(
            crate::environment::NAMES
                .iter()
                .any(|name| name.name == key)
                || key == "CUSTOM"
        );
    }
}

#[test]
fn pane_identity_can_be_removed_without_removing_workspace_identity() {
    let mut command = maestro_process::shell::pane_custom_command_pty_builder("");
    let launch = PaneLaunchEnv::from_extra(vec![
        ("MAESTRO_PANE_ID".into(), "outer".into()),
        ("MAESTRO_WORKSPACE_ID".into(), "workspace".into()),
    ])
    .without_pane_identity();
    launch.apply(&mut command, Path::new("/tmp/maestro.sock"), None);
    assert_eq!(value(&command, "MAESTRO_PANE_ID"), None);
    assert_eq!(
        value(&command, "MAESTRO_WORKSPACE_ID"),
        Some("workspace".into())
    );
}

/// Outer-terminal markers a pane child must not inherit.
const OUTER_TERMINAL_MARKERS: [&str; 12] = [
    "ITERM_SESSION_ID",
    "LC_TERMINAL",
    "LC_TERMINAL_VERSION",
    "WEZTERM_PANE",
    "KITTY_WINDOW_ID",
    "WT_SESSION",
    "TMUX",
    "TMUX_PANE",
    "STY",
    "ZELLIJ",
    "ZELLIJ_SESSION_NAME",
    "ZELLIJ_PANE_ID",
];

/// How the pane command receives its parent environment.
#[derive(Clone, Copy, Debug)]
enum Mode {
    /// Edits over the parent's variables.
    Inherit,
    /// An empty environment holding only the parent's variables as values.
    Replace,
}

/// Apply the policy over an injected parent environment and return what the child would see.
fn effective(
    mode: Mode,
    launch: &PaneLaunchEnv,
    parent: &[(&str, &str)],
) -> BTreeMap<OsString, OsString> {
    let lookup = |name: &OsStr| {
        parent
            .iter()
            .find(|(key, _)| name == *key)
            .map(|(_, value)| OsString::from(value))
    };
    let mut command = maestro_process::shell::pane_custom_command_pty_builder("");
    let mut seen: BTreeMap<OsString, OsString> = parent
        .iter()
        .map(|(key, value)| ((*key).into(), (*value).into()))
        .collect();
    if let Mode::Replace = mode {
        command.environment = PtyEnvironment::Replace(seen.clone().into_iter().collect());
    }
    launch.apply_with(&mut command, Path::new("/tmp/maestro.sock"), None, &lookup);
    match command.environment {
        PtyEnvironment::Inherit(edits) => {
            for (key, value) in edits {
                match value {
                    Some(value) => seen.insert(key, value),
                    None => seen.remove(&key),
                };
            }
            seen
        }
        PtyEnvironment::Replace(values) => values.into_iter().collect(),
    }
}

/// Run `check` once per environment mode.
fn each_mode(check: impl Fn(Mode)) {
    for mode in [Mode::Inherit, Mode::Replace] {
        check(mode);
    }
}

#[test]
fn outer_terminal_markers_are_not_inherited_by_panes() {
    each_mode(|mode| {
        let mut parent: Vec<(&str, &str)> = OUTER_TERMINAL_MARKERS
            .iter()
            .map(|name| (*name, "outer"))
            .collect();
        parent.push(("CODEX_THREAD_ID", "outer"));
        parent.push(("UNRELATED", "kept"));
        let launch = PaneLaunchEnv::from_extra(vec![("CUSTOM".into(), "yes".into())])
            .with_identity("workspace".into(), "tab".into(), "pane".into());
        let seen = effective(mode, &launch, &parent);
        for name in OUTER_TERMINAL_MARKERS.iter().chain(&["CODEX_THREAD_ID"]) {
            assert!(!seen.contains_key(OsStr::new(name)), "{mode:?} kept {name}");
        }
        for (name, expected) in [
            ("UNRELATED", "kept"),
            ("CUSTOM", "yes"),
            ("MAESTRO_ENV", "1"),
            ("MAESTRO_WORKSPACE_ID", "workspace"),
            ("MAESTRO_TAB_ID", "tab"),
            ("MAESTRO_PANE_ID", "pane"),
        ] {
            assert_eq!(
                seen.get(OsStr::new(name)),
                Some(&expected.into()),
                "{mode:?} {name}"
            );
        }
    });
}

#[test]
fn explicit_environment_can_restore_an_outer_terminal_marker() {
    each_mode(|mode| {
        let launch = PaneLaunchEnv::from_extra(vec![("TMUX".into(), "user".into())]);
        let seen = effective(mode, &launch, &[("TMUX", "outer")]);
        assert_eq!(
            seen.get(OsStr::new("TMUX")),
            Some(&"user".into()),
            "{mode:?}"
        );
    });
}

#[test]
fn panes_name_maestro_and_its_version_as_their_terminal_program() {
    each_mode(|mode| {
        let parent = [
            ("TERM_PROGRAM", "iTerm.app"),
            ("TERM_PROGRAM_VERSION", "3.5.0"),
        ];
        let seen = effective(mode, &PaneLaunchEnv::default(), &parent);
        let version = maestro_protocol::build_info::version();
        assert!(!version.is_empty());
        assert_eq!(
            seen.get(OsStr::new("TERM_PROGRAM")),
            Some(&"maestro".into()),
            "{mode:?}"
        );
        assert_eq!(
            seen.get(OsStr::new("TERM_PROGRAM_VERSION")),
            Some(&version.into()),
            "{mode:?}"
        );
    });
}

#[test]
fn panes_turn_fish_reflow_handling_off_unless_the_parent_chose() {
    each_mode(|mode| {
        let launch = PaneLaunchEnv::default();
        let name = OsStr::new("fish_handle_reflow");
        assert_eq!(
            effective(mode, &launch, &[]).get(name),
            Some(&"0".into()),
            "{mode:?} default"
        );
        let inherited = effective(mode, &launch, &[("fish_handle_reflow", "1")]);
        assert_eq!(inherited.get(name), Some(&"1".into()), "{mode:?} inherited");
        let cased = effective(mode, &launch, &[("FISH_HANDLE_REFLOW", "1")]);
        assert_eq!(cased.get(name), Some(&"0".into()), "{mode:?} other casing");
        assert_eq!(
            cased.get(OsStr::new("FISH_HANDLE_REFLOW")),
            Some(&"1".into()),
            "{mode:?}"
        );
    });
}

#[cfg(unix)]
#[test]
fn process_variable_reads_the_real_environment_by_exact_name() {
    let path = OsStr::new("PATH");
    assert_eq!(process_variable(path), std::env::var_os(path));
    assert!(process_variable(path).is_some());
    let flipped = OsStr::new("path");
    assert_eq!(process_variable(flipped), std::env::var_os(flipped));
}
