//! Termination signals of a serving process: cleanup and exit on the first, the native
//! default action on a second one during shutdown.
#![cfg(unix)]
use crate::support::{Child, Served, serve};
use maestro_extensions::{ExtensionFactory, types::events::SessionShutdownEvent};
use maestro_models::Local;
use maestro_test_directory::Scratch;
use maestro_test_process::{CASE, TestProcess, selection};
use std::convert::Infallible;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::symlink;
use std::os::unix::process::ExitStatusExt;
use std::process::Command;
use std::rc::Rc;

/// The case that runs as the process of a bash command.
const HELD: &str = "held";

/// How a `session_shutdown` handler behaves once it reports the shutdown.
#[derive(Clone, Copy)]
enum Disposal {
    /// It returns.
    Finishes,
    /// It waits forever without blocking the executor.
    Waits,
    /// It blocks the executor's thread.
    Blocks,
}

impl Disposal {
    /// Blocks the thread forever when the handler blocks.
    fn block(self) {
        while let Self::Blocks = self {
            std::thread::park();
        }
    }

    /// Waits forever when the handler waits.
    async fn wait(self) {
        if let Self::Waits = self {
            std::future::pending::<()>().await;
        }
    }
}

/// An extension whose `session_shutdown` handler reports the shutdown on standard error,
/// then behaves as `disposal` says.
fn disposing(disposal: Disposal) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let shutdown = api.on::<SessionShutdownEvent, ()>(
            "session_shutdown".into(),
            Rc::new(move |_, _| {
                eprintln!("disposing");
                disposal.block();
                Box::pin(async move {
                    disposal.wait().await;
                    Ok(None)
                })
            }),
        );
        Box::pin(std::future::ready(shutdown))
    })
}

/// The child's disposal for the selected case.
fn served() -> Served {
    let selected = selection();
    let disposal = [("waits", Disposal::Waits), ("blocks", Disposal::Blocks)]
        .into_iter()
        .find_map(|(case, disposal)| selected.runs_case(case).then_some(disposal))
        .unwrap_or(Disposal::Finishes);
    Served {
        factories: vec![disposing(disposal)],
        ..Served::default()
    }
}

/// The process of a bash command, in the working directory the test prepared: it keeps the
/// FIFO `report` open for writing until it ends and reports that it runs, then reports that
/// it is alive once the test closes the FIFO `release`.
fn hold() {
    let mut report = File::options().write(true).open("report").unwrap();
    report.write_all(b"running\n").unwrap();
    File::open("release")
        .unwrap()
        .read_to_end(&mut Vec::new())
        .unwrap();
    report.write_all(b"alive\n").unwrap();
}

/// A first SIGTERM or SIGHUP kills the shell commands still running, disposes the session
/// and exits with 143 or 129.
#[test]
fn signals_clean_children_before_exit() {
    let selected = selection();
    if selected.case() == Some(HELD) {
        hold();
        return;
    }
    if selected.is_child() {
        serve(served());
    }
    // The command names this executable through a link in its working directory, so it
    // holds no path to quote.
    let command = TestProcess::current("signals_clean_children_before_exit")
        .unwrap()
        .nocapture()
        .invoke_with(|arguments| {
            Ok::<_, Infallible>(format!("{CASE}={HELD} exec ./held {}", arguments.join(" ")))
        })
        .unwrap();
    let line = serde_json::json!({"id": "b", "type": "bash", "command": command});
    for (signal, code) in [("TERM", 143), ("HUP", 129)] {
        let scratch = Scratch::new().unwrap();
        let dir = scratch.path();
        symlink(std::env::current_exe().unwrap(), dir.join("held")).unwrap();
        let made = Command::new("mkfifo")
            .args(["report", "release"])
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(made.success());
        let mut child = Child::start("signals_clean_children_before_exit", "serve", dir);
        child.send(format!("{line}\n").as_bytes());
        let mut report = BufReader::new(File::open(dir.join("report")).unwrap());
        let mut running = String::new();
        report.read_line(&mut running).unwrap();
        assert_eq!(running, "running\n", "{signal}");
        let release = File::options()
            .write(true)
            .open(dir.join("release"))
            .unwrap();
        child.signal(signal);
        let (status, errors) = child.wait_open();
        assert_eq!(status.code(), Some(code), "{signal}");
        assert_eq!(errors, "disposing\n", "{signal}");
        // The server sent its kill before it exited, and a killed process runs none of its own
        // code again, so only a process nothing killed reads the release and reports. The
        // report ends once the held process has ended.
        drop(release);
        let mut rest = String::new();
        report.read_to_string(&mut rest).unwrap();
        assert_eq!(rest, "", "{signal}: the running shell command was killed");
    }
}

/// Once shutdown has started, a second signal of either kind kills the process by that
/// signal, also while disposal waits or blocks the executor.
#[test]
fn second_signal_has_native_signal_death() {
    if selection().is_child() {
        serve(served());
    }
    for (case, signal, number) in [
        ("waits", "HUP", 1),
        ("blocks", "TERM", 15),
        ("waits", "TERM", 15),
    ] {
        let scratch = Scratch::new().unwrap();
        let mut child = Child::start(
            "second_signal_has_native_signal_death",
            case,
            scratch.path(),
        );
        child.send(b"{\"id\":\"s\",\"type\":\"get_state\"}\n");
        assert_eq!(child.next()["id"], "s");
        child.signal(signal);
        assert_eq!(child.next_error_line(), "disposing\n");
        child.signal(signal);
        let (status, _) = child.wait_open();
        assert_eq!(
            (status.code(), status.signal()),
            (None, Some(number)),
            "{case} {signal}"
        );
    }
}
