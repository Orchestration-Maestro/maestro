//! Launch edits observed by actual shell children.
use super::*;
use maestro_process::pty::{PtyCommand, PtyEnvironment};
use std::{ffi::OsString, path::Path, process::Command};

/// Execute the environment produced by the pane launch policy.
fn capture(extra: Vec<(String, String)>, script: &str) -> String {
    let mut description = maestro_process::shell::pane_custom_command_pty_builder(script);
    description.environment = PtyEnvironment::Replace(vec![
        ("TERM".into(), "outer-terminal".into()),
        ("COLORTERM".into(), "outer-color".into()),
        ("CODEX_THREAD_ID".into(), "outer-session".into()),
        ("MAESTRO_STARTUP_CWD".into(), "/outer-startup".into()),
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
fn startup_directory_is_not_inherited_by_panes() {
    assert_eq!(
        capture(vec![], "printf '%s' \"${MAESTRO_STARTUP_CWD-unset}\""),
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
