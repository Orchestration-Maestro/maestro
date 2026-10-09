//! Controlled native shell routing and launch behavior.
#![cfg(not(target_arch = "wasm32"))]
use super::resolve_config_value::{
    ConfigValueOperations, clear_config_value_cache,
    native::{Attempt, Launch, Platform, default_shell, routed, spawn_failure},
    resolve_config_value, resolve_config_value_uncached,
};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    io,
};

/// Route a controlled configured attempt through the shared value resolver.
struct Controlled {
    /// Selection failure rather than a configured tuple.
    selection_fails: bool,
    /// Attempts in execution order.
    attempts: RefCell<VecDeque<Attempt>>,
    /// Observed executable/argument identity.
    launches: RefCell<Vec<Launch>>,
    /// Count lazy selection reads.
    selections: Cell<usize>,
    /// Platform under observation.
    windows: bool,
}
impl ConfigValueOperations for Controlled {
    fn environment(&self, _: &str) -> Option<String> {
        None
    }
    fn execute(&self, command: &str) -> Option<Vec<u8>> {
        routed(
            self.windows,
            command,
            &|| {
                self.selections.set(self.selections.get() + 1);
                if self.selection_fails {
                    Err(io::Error::other("selection failed"))
                } else {
                    Ok((
                        "configured".into(),
                        vec!["-x".into(), "-x".into(), "argument with spaces".into()],
                    ))
                }
            },
            &|launch| {
                self.launches.borrow_mut().push(launch);
                self.attempts.borrow_mut().pop_front().unwrap()
            },
            || {
                default_shell(
                    command,
                    if self.windows {
                        Platform::Windows
                    } else {
                        Platform::Unix
                    },
                    None,
                )
            },
        )
    }
}
/// Build a controlled process observation.
fn controlled(windows: bool, attempts: Vec<Attempt>) -> Controlled {
    Controlled {
        selection_fails: false,
        attempts: RefCell::new(attempts.into()),
        launches: RefCell::new(Vec::new()),
        selections: Cell::new(0),
        windows,
    }
}
#[test]
fn configured_shell_fallback_is_selective() {
    for (kind, fallback) in [
        (io::ErrorKind::NotFound, true),
        (io::ErrorKind::PermissionDenied, false),
        (io::ErrorKind::TimedOut, false),
    ] {
        let operations = controlled(
            true,
            vec![
                spawn_failure(&io::Error::from(kind)),
                Attempt::Completed(Some(b"fallback".to_vec())),
            ],
        );
        assert_eq!(
            resolve_config_value_uncached("!command", &operations).as_deref(),
            fallback.then_some("fallback")
        );
        assert_eq!(
            operations.launches.borrow().len(),
            if fallback { 2 } else { 1 }
        );
    }

    // Completed attempts preserve missing, empty and successful stdout.
    for output in [
        None,
        Some(Vec::new()),
        Some(b" \n".to_vec()),
        Some(b"key".to_vec()),
    ] {
        let expected = output
            .as_ref()
            .filter(|bytes| bytes.as_slice() == b"key")
            .map(|_| "key");
        let operations = controlled(true, vec![Attempt::Completed(output)]);
        assert_eq!(
            resolve_config_value_uncached("!command", &operations).as_deref(),
            expected
        );
        assert_eq!(operations.launches.borrow().len(), 1);
        assert_eq!(operations.selections.get(), 1);
    }
    let mut operations = controlled(true, vec![Attempt::Completed(Some(b"fallback".to_vec()))]);
    operations.selection_fails = true;
    assert_eq!(
        resolve_config_value_uncached("!command", &operations).as_deref(),
        Some("fallback")
    );
    assert_eq!(operations.launches.borrow().len(), 1);
    let operations = controlled(true, vec![Attempt::Unavailable, Attempt::Completed(None)]);
    assert_eq!(resolve_config_value_uncached("!command", &operations), None);
    assert_eq!(operations.launches.borrow().len(), 2);
}
#[test]
fn unix_helpers_do_not_select_configured_shell() {
    clear_config_value_cache();
    let mut operations = controlled(false, vec![Attempt::Completed(Some(b"key".to_vec()))]);
    operations.selection_fails = true;
    assert_eq!(
        resolve_config_value_uncached("", &operations).as_deref(),
        Some("")
    );
    assert_eq!(
        resolve_config_value("!!command", &operations).as_deref(),
        Some("key")
    );
    assert_eq!(
        resolve_config_value("!!command", &operations).as_deref(),
        Some("key")
    );
    assert_eq!(operations.launches.borrow().len(), 1);
    assert_eq!(operations.selections.get(), 0);
    assert_eq!(operations.launches.borrow()[0].executable, "/bin/sh");
    assert_eq!(operations.launches.borrow()[0].args, ["-c", "!command"]);
}
#[test]
fn helper_launch_preserves_argv_and_stream_policy() {
    let operations = controlled(true, vec![Attempt::Completed(Some(b"result".to_vec()))]);
    assert_eq!(
        resolve_config_value_uncached("!printf \"one | two\"", &operations).as_deref(),
        Some("result")
    );
    assert_eq!(
        &*operations.launches.borrow(),
        &[Launch {
            executable: "configured".into(),
            args: vec![
                "-x".into(),
                "-x".into(),
                "argument with spaces".into(),
                "printf \"one | two\"".into()
            ],
            verbatim: false
        }]
    );
    #[cfg(unix)]
    {
        let operations =
            super::resolve_config_value::ProcessConfigValueOperations::new(|| unreachable!());
        assert_eq!(
            resolve_config_value_uncached(
                "!if read line; then printf wrong; else printf key; fi; printf noise >&2",
                &operations
            )
            .as_deref(),
            Some("key")
        );
    }
}
#[test]
fn default_shell_preserves_command_quoting() {
    let text = "echo \"one | two\" | cat";
    for (android, executable) in [(false, "/bin/sh"), (true, "/system/bin/sh")] {
        assert_eq!(
            default_shell(
                text,
                if android {
                    Platform::Android
                } else {
                    Platform::Unix
                },
                Some("ignored")
            ),
            Launch {
                executable: executable.into(),
                args: vec!["-c".into(), text.into()],
                verbatim: false
            }
        );
    }
    for shell in [
        None,
        Some(""),
        Some("cmd"),
        Some("CmD.ExE"),
        Some("C:\\windows\\CMD.EXE"),
    ] {
        let result = default_shell(text, Platform::Windows, shell);
        assert_eq!(
            result.executable,
            shell.filter(|value| !value.is_empty()).unwrap_or("cmd.exe")
        );
        assert_eq!(
            result.args,
            ["/d", "/s", "/c", "\"echo \"one | two\" | cat\""]
        );
        assert!(result.verbatim);
    }
    for shell in [
        "bash.exe",
        "notcmd.exe",
        "cmd.exe.bak",
        "C:/windows/cmd.exe",
        "cmd.exe ",
    ] {
        let result = default_shell(text, Platform::Windows, Some(shell));
        assert_eq!(result.args, ["-c", text]);
        assert!(!result.verbatim);
    }
}
