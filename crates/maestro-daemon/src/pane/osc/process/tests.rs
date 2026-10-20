//! Foreground process history and theme policy.

use super::*;
use maestro_process::pty::foreground::ForegroundProcess;

fn shell_job(shell_pid: u32) -> ForegroundJob {
    ForegroundJob {
        process_group_id: shell_pid,
        processes: vec![ForegroundProcess {
            pid: shell_pid,
            name: "zsh".into(),
            argv0: Some("zsh".into()),
            argv: Some(vec!["zsh".into()]),
            cmdline: Some("zsh".into()),
        }],
    }
}

#[test]
fn pane_application_scrollback_compat_matches_process_name_and_cmdline() {
    let name_only = ForegroundJob {
        process_group_id: 42,
        processes: vec![ForegroundProcess {
            pid: 42,
            name: "droid".to_string(),
            argv0: None,
            argv: Some(vec![
                "/opt/factory/droid".to_string(),
                "--resume".to_string(),
            ]),
            cmdline: Some("/opt/factory/droid --resume".to_string()),
        }],
    };
    assert!(foreground_job_uses_droid_scrollback_compat(&name_only));

    let cmdline_only = ForegroundJob {
        process_group_id: 42,
        processes: vec![ForegroundProcess {
            pid: 42,
            name: "bun".to_string(),
            argv0: Some("bun".to_string()),
            argv: Some(vec![
                "bun".to_string(),
                "/home/can/.local/bin/droid".to_string(),
                "--resume".to_string(),
            ]),
            cmdline: Some("/home/can/.local/bin/droid --resume".to_string()),
        }],
    };
    assert!(foreground_job_uses_droid_scrollback_compat(&cmdline_only));

    let shell = shell_job(7);
    assert!(!foreground_job_uses_droid_scrollback_compat(&shell));
}

#[test]
fn pane_strip_scrollback_clear_sequences_removes_ed3_only() {
    let filtered = strip_scrollback_clear_sequences(b"a\x1b[3Jb\x1b[?3Jc\x1b[2Jd");
    assert_eq!(filtered.as_ref(), b"abc\x1b[2Jd");
}

#[test]
fn pane_primary_screen_application_compat_ignores_scrollback_clear_only_for_droid() {
    let droid_job = ForegroundJob {
        process_group_id: 42,
        processes: vec![ForegroundProcess {
            pid: 42,
            name: "droid".to_string(),
            argv0: Some("droid".to_string()),
            argv: Some(vec!["droid".to_string()]),
            cmdline: Some("droid".to_string()),
        }],
    };

    let filtered =
        maybe_filter_primary_screen_scrollback_clear(b"\x1b[3J\x1b[2J", false, Some(&droid_job));
    assert_eq!(filtered.as_ref(), b"\x1b[2J");

    let shell =
        maybe_filter_primary_screen_scrollback_clear(b"\x1b[3J\x1b[2J", false, Some(&shell_job(7)));
    assert_eq!(shell.as_ref(), b"\x1b[3J\x1b[2J");

    let alternate =
        maybe_filter_primary_screen_scrollback_clear(b"\x1b[3J\x1b[2J", true, Some(&droid_job));
    assert_eq!(alternate.as_ref(), b"\x1b[3J\x1b[2J");
}

#[test]
fn pane_host_theme_restore_waits_for_shell_and_non_alternate_screen() {
    assert!(!should_restore_host_terminal_theme(
        42,
        7,
        true,
        Some(&shell_job(7)),
    ));
    assert!(!should_restore_host_terminal_theme(42, 7, false, None));
    assert!(!should_restore_host_terminal_theme(
        42,
        7,
        false,
        Some(&ForegroundJob {
            process_group_id: 42,
            processes: vec![ForegroundProcess {
                pid: 42,
                name: "droid".to_string(),
                argv0: Some("droid".to_string()),
                argv: Some(vec!["droid".to_string()]),
                cmdline: Some("droid".to_string()),
            }],
        }),
    ));
    assert!(should_restore_host_terminal_theme(
        42,
        7,
        false,
        Some(&shell_job(7)),
    ));

    #[cfg(target_os = "macos")]
    assert!(should_restore_host_terminal_theme(
        7,
        7,
        false,
        Some(&shell_job(7)),
    ));

    #[cfg(not(target_os = "macos"))]
    assert!(!should_restore_host_terminal_theme(
        7,
        7,
        false,
        Some(&shell_job(7)),
    ));
}
