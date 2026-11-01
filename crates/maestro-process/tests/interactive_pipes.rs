//! Children whose three standard streams the caller drives directly.
#![cfg(unix)]
use crate::child_programs::{self, async_command};
use maestro_process::child_process::{CapturedChild, Termination};
use std::os::unix::process::ExitStatusExt;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

/// A runtime with the process and timer drivers.
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

/// Reads output lines up to the child's `ready` line, past the harness start lines.
async fn read_until_ready(output: &mut BufReader<tokio::process::ChildStdout>) {
    loop {
        let mut line = String::new();
        assert_ne!(output.read_line(&mut line).await.unwrap(), 0);
        if line == "ready\n" {
            return;
        }
    }
}

#[test]
fn interactive_pipes_keep_native_command_and_stream_ownership() {
    let dir = std::env::temp_dir().canonicalize().unwrap();
    let mut command = async_command(child_programs::line_answerer, &[("PROBE", "set")]);
    command.args(["two words", "$HOME"]).current_dir(&dir);
    runtime().block_on(async {
        let mut child = CapturedChild::spawn_piped(&mut command).unwrap();
        let mut input = child.take_stdin().unwrap();
        let mut output = BufReader::new(child.take_stdout().unwrap());
        let mut errors = child.take_stderr().unwrap();
        assert!(child.take_stdin().is_none());
        assert!(child.take_stdout().is_none());
        assert!(child.take_stderr().is_none());
        read_until_ready(&mut output).await;
        assert!(child.try_wait().unwrap().is_none());
        input.write_all(b"hello\n").await.unwrap();
        let mut rest = String::new();
        output.read_to_string(&mut rest).await.unwrap();
        assert_eq!(rest, format!("hello|two words|$HOME|{}|set", dir.display()));
        let mut diagnostic = Vec::new();
        errors.read_to_end(&mut diagnostic).await.unwrap();
        assert_eq!(diagnostic, b"diag\xff");
        assert_eq!(child.wait().await.unwrap().code(), Some(3));
        assert_eq!(child.try_wait().unwrap().and_then(|s| s.code()), Some(3));
        assert_eq!(child.id(), None);
    });
}

#[test]
fn taken_pipes_keep_termination_and_reaping_policy() {
    let escalate = Termination::TerminateThenKill {
        after: Duration::from_millis(1000),
    };
    runtime().block_on(async {
        for (survives, signal) in [(false, 15), (true, 9)] {
            let variables: &[(&str, &str)] = if survives {
                &[("CHILD_SURVIVES", "1")]
            } else {
                &[]
            };
            let mut command = async_command(child_programs::terminable, variables);
            let mut child = CapturedChild::spawn_piped(&mut command).unwrap();
            let _input = child.take_stdin().unwrap();
            let mut output = BufReader::new(child.take_stdout().unwrap());
            read_until_ready(&mut output).await;
            child.terminate(escalate).await.unwrap();
            assert_eq!(child.wait().await.unwrap().signal(), Some(signal));
            assert_eq!(child.id(), None);
        }
        let mut capture =
            CapturedChild::spawn(&mut async_command(child_programs::immediate_exit, &[])).unwrap();
        assert!(capture.take_stdin().is_none());
        // The harness start bytes need a live reader until the child ends.
        let stdout = capture.take_stdout();
        assert!(stdout.is_some());
        assert!(capture.wait().await.unwrap().success());
        drop(stdout);
    });
}
