//! Text clipboard routing through controlled application effects.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use futures_util::future::LocalBoxFuture;
use maestro_app::utils::clipboard_native::{ClipboardModule, clipboard};
use std::{cell::Cell, io};

/// Admission-only module; constructing it never touches the system clipboard.
struct Module;
impl ClipboardModule for Module {
    fn has_image(&self) -> io::Result<bool> {
        unreachable!("text operation must not probe images")
    }
    fn get_image_binary(&self) -> LocalBoxFuture<'_, io::Result<Option<Vec<u8>>>> {
        unreachable!("text operation must not read images")
    }
    fn set_text<'a>(&'a self, _: &'a str) -> LocalBoxFuture<'a, io::Result<()>> {
        unreachable!("admission must not write")
    }
}

#[test]
fn native_admission_respects_termux_and_display() {
    for case in controlled::corpus().availability {
        let calls = Cell::new(0);
        let result = clipboard(
            controlled::platform(&case.platform),
            |key| case.env.get(key).cloned(),
            || {
                calls.set(calls.get() + 1);
                (!case.load_fails).then_some(Module)
            },
        );
        assert_eq!(result.is_some(), case.present);
        assert_eq!(calls.get(), case.loads);
    }
}

#[path = "support/clipboard_effects.rs"]
mod controlled;

#[test]
fn native_copy_finishes_without_fallback() {
    controlled::run("native_copy_finishes_without_fallback");
}

#[test]
fn remote_copy_waits_for_native_before_terminal() {
    controlled::run("remote_copy_waits_for_native_before_terminal");
}

#[test]
fn linux_copy_never_calls_native() {
    controlled::run("linux_copy_never_calls_native");
}

#[test]
fn other_platforms_keep_tool_and_terminal_fallbacks() {
    controlled::run("other_platforms_keep_tool_and_terminal_fallbacks");
}

#[test]
fn platform_tools_preserve_input_and_launch_policy() {
    controlled::run("platform_tools_preserve_input_and_launch_policy");
}

#[test]
fn x11_tries_xclip_before_xsel() {
    controlled::run("x11_tries_xclip_before_xsel");
}

#[test]
fn termux_precedes_display_helpers() {
    controlled::run("termux_precedes_display_helpers");
}

#[test]
fn termux_failure_keeps_display_fallbacks() {
    controlled::run("termux_failure_keeps_display_fallbacks");
}

#[test]
fn terminal_copy_recovers_from_failed_tools() {
    controlled::run("terminal_copy_recovers_from_failed_tools");
}

#[test]
fn wayland_copy_requires_a_display_and_starts_detached() {
    controlled::run("wayland_copy_requires_a_display_and_starts_detached");
}

#[test]
fn wayland_start_failure_uses_x11_when_present() {
    controlled::run("wayland_start_failure_uses_x11_when_present");
}

#[test]
fn remote_tool_copy_also_emits_terminal_sequence() {
    controlled::run("remote_tool_copy_also_emits_terminal_sequence");
}

#[test]
fn terminal_sequence_uses_exact_utf8_and_padded_base64() {
    controlled::run("terminal_sequence_uses_exact_utf8_and_padded_base64");
}

#[test]
fn terminal_limit_counts_encoded_bytes() {
    controlled::run("terminal_limit_counts_encoded_bytes");
}

#[test]
fn oversized_terminal_does_not_undo_native_success() {
    controlled::run("oversized_terminal_does_not_undo_native_success");
}

#[test]
fn terminal_backpressure_is_not_copy_failure() {
    controlled::run("terminal_backpressure_is_not_copy_failure");
}

#[test]
fn terminal_write_failure_is_propagated() {
    controlled::run("terminal_write_failure_is_propagated");
}

#[test]
fn wayland_detection_uses_nonempty_display_or_exact_session_type() {
    for case in controlled::corpus().wayland {
        assert_eq!(
            maestro_app::utils::clipboard_image::is_wayland_session(|key| case
                .env
                .get(key)
                .cloned()),
            case.result
        );
    }
}

/// Child mode keeps all environment changes outside the parallel parent process.
#[cfg(target_os = "linux")]
#[test]
fn native_adapter_forwards_launches_without_clipboard_access() {
    use maestro_app::copy_to_clipboard;
    if std::env::var_os("MAESTRO_CLIPBOARD_CHILD").is_some() {
        crate::support::run_io(copy_to_clipboard("a\0é🙂")).unwrap();
        return;
    }
    let output = maestro_test_process::TestProcess::current(
        "native_adapter_forwards_launches_without_clipboard_access",
    )
    .unwrap()
    .nocapture()
    .env("MAESTRO_CLIPBOARD_CHILD", "1")
    .env_remove("SSH_CONNECTION")
    .env_remove("SSH_CLIENT")
    .env_remove("MOSH_CONNECTION")
    .env_remove("TERMUX_VERSION")
    .env_remove("DISPLAY")
    .env_remove("WAYLAND_DISPLAY")
    .env_remove("XDG_SESSION_TYPE")
    .output()
    .unwrap();
    assert!(output.status.success());

    let frame = b"\x1b]52;c;YQDDqfCfmYI=\x07";
    assert_eq!(
        output
            .stdout
            .windows(frame.len())
            .filter(|w| *w == frame)
            .count(),
        1
    );
    native_launches();
}

/// Exercises argv, bytes, status and independent descriptor forwarding.
#[cfg(target_os = "linux")]
fn native_launches() {
    use maestro_app::utils::clipboard::{ClipboardOperations, NativeClipboardOperations};
    use maestro_app::{OutputMode, SpawnSyncOptions};
    use tokio::process::Command;
    for (stdout, stderr) in [
        (OutputMode::Capture, OutputMode::Discard),
        (OutputMode::Discard, OutputMode::Capture),
    ] {
        let mut command = Command::new("python3");
        command.args(["-c", "import sys; sys.stdout.buffer.write(sys.stdin.buffer.read()); sys.stderr.write(sys.argv[1]); sys.exit(7)", "diagnostic"]);
        let output = NativeClipboardOperations
            .spawn_sync(
                &mut command,
                SpawnSyncOptions::Capture {
                    max_buffer_bytes: None,
                    stdout,
                    stderr,
                    input: Some(b"a\0\r\n"),
                    timeout: None,
                },
            )
            .unwrap();
        assert_eq!(output.status.code(), Some(7));
        assert_eq!(
            output.stdout,
            if stdout == OutputMode::Capture {
                &b"a\0\r\n"[..]
            } else {
                b""
            }
        );
        assert_eq!(
            output.stderr,
            if stderr == OutputMode::Capture {
                &b"diagnostic"[..]
            } else {
                b""
            }
        );
    }
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let mut command = Command::new("python3");
    command.args(["-c", "import socket,sys; s=socket.create_connection(('127.0.0.1',int(sys.argv[1]))); s.sendall(sys.stdin.buffer.read()); s.close()", &listener.local_addr().unwrap().port().to_string()]);
    NativeClipboardOperations
        .spawn(command, Some(b"background\0".to_vec()))
        .unwrap();
    let (mut stream, _) = listener.accept().unwrap();
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut stream, &mut bytes).unwrap();
    assert_eq!(bytes, b"background\0");
}
