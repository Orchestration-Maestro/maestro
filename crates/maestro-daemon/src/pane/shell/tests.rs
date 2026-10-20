//! Shell selection and command operands with supplied environments.
use super::*;

#[test]
fn configured_shell_wins_over_inherited_shell() {
    assert_eq!(
        pane_shell_from(
            " /usr/bin/nu ",
            Some("/bin/bash"),
            ShellLaunchTarget::OtherUnix
        ),
        "/usr/bin/nu"
    );
}

use crate::settings::model::ShellModeConfig::{self, Auto, Login, NonLogin};
use ShellLaunchTarget::{Macos, OtherUnix, Windows};
use std::{ffi::OsStr, io};

/// Build a shell with no inherited environment.
fn build(
    shell: &str,
    mode: ShellModeConfig,
    target: ShellLaunchTarget,
) -> io::Result<maestro_process::pty::PtyCommand> {
    pane_shell_command_builder_for_target(PaneShellConfig::new(shell, mode), target, |_| None)
}

#[test]
fn inherited_shell_is_the_unix_fallback() {
    assert_eq!(
        pane_shell_from("", Some(" /bin/bash "), OtherUnix),
        "/bin/bash"
    );
}

#[test]
fn windows_uses_its_default_instead_of_shell_variable() {
    assert_eq!(
        pane_shell_from("", Some("cmd.exe"), Windows),
        "powershell.exe"
    );
}

#[test]
fn blank_shell_values_use_the_platform_default() {
    assert_eq!(pane_shell_from("  ", Some("  "), OtherUnix), "/bin/sh");
    assert_eq!(pane_shell_from("", None, OtherUnix), "/bin/sh");
}

#[test]
fn automatic_login_is_limited_to_macos() {
    for (mode, target, expected) in [
        (Auto, Macos, true),
        (Auto, OtherUnix, false),
        (Auto, Windows, false),
        (Login, OtherUnix, true),
        (NonLogin, Macos, false),
    ] {
        assert_eq!(shell_mode_uses_login_shell(mode, target), expected);
    }
}

#[test]
fn explicit_login_sets_argument_zero_and_shell_environment() {
    let command = build("/bin/sh", Login, OtherUnix).unwrap();
    assert_eq!(command.program, "/bin/sh");
    assert_eq!(command.argv0.as_deref(), Some(OsStr::new("-sh")));
    assert!(
        matches!(command.environment, maestro_process::pty::PtyEnvironment::Inherit(edits) if edits == vec![("SHELL".into(), Some("/bin/sh".into()))])
    );
}

#[test]
fn macos_automatic_mode_builds_a_login_launch() {
    let command = build("/bin/sh", Auto, Macos).unwrap();
    assert_eq!(command.argv0.as_deref(), Some(OsStr::new("-sh")));
}

#[test]
fn other_unix_automatic_mode_launches_directly() {
    let command = build("/bin/sh", Auto, OtherUnix).unwrap();
    assert_eq!(command.program, "/bin/sh");
    assert!(command.argv0.is_none());
    assert!(command.args.is_empty());
}

#[test]
fn windows_powershell_reports_prompt_directory_without_resetting_status() {
    for shell in [
        "powershell.exe",
        "pwsh.exe",
        "C:\\Program Files\\PowerShell\\7\\pwsh.exe",
    ] {
        let command = build(shell, NonLogin, Windows).unwrap();
        assert_eq!(command.program, shell);
        assert_eq!(
            command.args,
            [
                "-NoExit",
                "-Command",
                WINDOWS_POWERSHELL_SHELL_INTEGRATION_COMMAND
            ]
            .map(std::ffi::OsString::from)
        );
    }
    let script = WINDOWS_POWERSHELL_SHELL_INTEGRATION_COMMAND;
    assert!(
        script
            .find("[Environment]::CurrentDirectory = $loc.ProviderPath")
            .unwrap()
            < script.find("]9;9;").unwrap()
    );
    assert!(script.contains("$global:__MaestroOriginalPrompt = $function:prompt"));
    assert!(script.contains("$null -eq $global:__MaestroOriginalPrompt"));
    assert!(script.contains("'FileSystem'"));
    assert!(!script.contains('"'));
    assert!(
        script.find("@(& $global:__MaestroOriginalPrompt)").unwrap()
            < script.find("$loc =").unwrap()
    );
}

#[test]
fn windows_other_shell_has_no_prompt_wrapper() {
    let command = build("cmd.exe", NonLogin, Windows).unwrap();
    assert_eq!(command.program, "cmd.exe");
    assert!(command.args.is_empty());
}

#[test]
fn unix_powershell_has_no_windows_prompt_wrapper() {
    let command = build("pwsh", NonLogin, OtherUnix).unwrap();
    assert_eq!(command.program, "pwsh");
    assert!(command.args.is_empty());
}

#[test]
fn prompt_integration_requires_windows_direct_powershell() {
    for (shell, mode, target, expected) in [
        ("pwsh.exe", NonLogin, Windows, true),
        ("pwsh.exe", NonLogin, OtherUnix, false),
        ("pwsh.exe", NonLogin, Macos, false),
        ("pwsh.exe", Login, Windows, false),
        ("cmd.exe", NonLogin, Windows, false),
    ] {
        assert_eq!(
            uses_windows_powershell_pane_shell_for_target(
                PaneShellConfig::new(shell, mode),
                target,
                None
            ),
            expected
        );
    }
}

#[test]
fn missing_login_shell_is_an_error_not_a_fallback() {
    assert_eq!(
        build("/no/such/maestro-shell", Login, OtherUnix)
            .unwrap_err()
            .kind(),
        io::ErrorKind::NotFound
    );
}

#[test]
fn bare_login_shell_is_resolved_using_supplied_path() {
    let command = pane_shell_command_builder_for_target(
        PaneShellConfig::new("sh", Login),
        OtherUnix,
        |key| (key == "PATH").then(|| "/bin".into()),
    )
    .unwrap();
    assert_eq!(command.program, "/bin/sh");
    assert_eq!(command.argv0.as_deref(), Some(OsStr::new("-sh")));
}

#[test]
fn absolute_login_shell_path_is_preserved() {
    assert_eq!(
        resolve_shell_for_login_mode("/bin/sh", None).unwrap(),
        "/bin/sh"
    );
}

#[test]
fn non_login_shell_is_the_direct_executable() {
    let command = build("/bin/sh", NonLogin, OtherUnix).unwrap();
    assert_eq!(command.program, "/bin/sh");
    assert!(command.argv0.is_none());
    assert!(command.args.is_empty());
}

#[test]
fn windows_login_uses_command_processor_without_unix_argument_zero() {
    let config = PaneShellConfig::new("/bin/sh", Login);
    let command = pane_shell_command_builder_for_target(config, Windows, |key| {
        (key == "ComSpec").then(|| "custom-cmd.exe".into())
    })
    .unwrap();
    assert_eq!(command.program, "custom-cmd.exe");
    assert!(command.argv0.is_none());
    assert!(command.args.is_empty());
    assert_eq!(build("/bin/sh", Login, Windows).unwrap().program, "cmd.exe");
}
