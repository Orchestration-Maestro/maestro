//! Children whose three standard streams the caller drives directly.
#![cfg(unix)]
use maestro_process::child_process::{CapturedChild, Termination};
use std::os::unix::process::ExitStatusExt;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

/// A runtime with the process and timer drivers.
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

#[test]
fn interactive_pipes_keep_native_command_and_stream_ownership() {
    let dir = std::env::temp_dir().canonicalize().unwrap();
    let script = r"
import os, sys
sys.stdout.write('ready\n'); sys.stdout.flush()
line = sys.stdin.readline()
sys.stderr.buffer.write(b'diag\xff'); sys.stderr.flush()
sys.stdout.write('|'.join([line.strip(), *sys.argv[1:], os.getcwd(), os.environ['PROBE']]))
sys.exit(3)
";
    let mut command = Command::new("python3");
    command
        .args(["-c", script, "two words", "$HOME"])
        .current_dir(&dir)
        .env("PROBE", "set");
    runtime().block_on(async {
        let mut child = CapturedChild::spawn_piped(&mut command).unwrap();
        let mut input = child.take_stdin().unwrap();
        let mut output = BufReader::new(child.take_stdout().unwrap());
        let mut errors = child.take_stderr().unwrap();
        assert!(child.take_stdin().is_none());
        assert!(child.take_stdout().is_none());
        assert!(child.take_stderr().is_none());
        let mut ready = String::new();
        output.read_line(&mut ready).await.unwrap();
        assert_eq!(ready, "ready\n");
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
        for (handler, signal) in [("signal.SIG_DFL", 15), ("signal.SIG_IGN", 9)] {
            let script = format!(
                "import signal, sys\nsignal.signal(signal.SIGTERM, {handler})\n\
                 print('ready', flush=True)\nsys.stdin.read()"
            );
            let mut child =
                CapturedChild::spawn_piped(Command::new("python3").args(["-c", &script])).unwrap();
            let _input = child.take_stdin().unwrap();
            let mut output = BufReader::new(child.take_stdout().unwrap());
            let mut ready = String::new();
            output.read_line(&mut ready).await.unwrap();
            assert_eq!(ready, "ready\n");
            child.terminate(escalate).await.unwrap();
            assert_eq!(child.wait().await.unwrap().signal(), Some(signal));
            assert_eq!(child.id(), None);
        }
        let mut capture = CapturedChild::spawn(Command::new("python3").args(["-c", ""])).unwrap();
        assert!(capture.take_stdin().is_none());
        assert!(capture.take_stdout().is_some());
        assert!(capture.wait().await.unwrap().success());
    });
}
