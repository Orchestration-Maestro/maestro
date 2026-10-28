//! Termination signals of a serving process: cleanup and exit on the first, the native
//! default action on a second one during shutdown.
#![cfg(unix)]
use crate::support::{Child, Served, serve};
use maestro_app::{ExtensionFactory, Local, SessionShutdownEvent};
use maestro_test_directory::Scratch;
use maestro_test_process::selection;
use std::os::unix::process::ExitStatusExt;
use std::rc::Rc;

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

/// Whether the process `pid` has ended: gone, or a zombie nobody reaped yet.
fn ended(pid: &str) -> bool {
    let state = std::process::Command::new("ps")
        .args(["-o", "stat=", "-p", pid])
        .output()
        .unwrap()
        .stdout;
    let state = String::from_utf8(state).unwrap();
    state.trim().is_empty() || state.trim().starts_with('Z')
}

/// A first SIGTERM or SIGHUP kills the shell commands still running, disposes the session
/// and exits with 143 or 129.
#[test]
fn signals_clean_children_before_exit() {
    if selection().is_child() {
        serve(served());
    }
    for (signal, code) in [("TERM", 143), ("HUP", 129)] {
        let scratch = Scratch::new().unwrap();
        let fifo = scratch.path().join("pid");
        let made = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap();
        assert!(made.success());
        let mut child = Child::start(
            "signals_clean_children_before_exit",
            "serve",
            scratch.path(),
        );
        let command = format!("echo $$ > {}; exec sleep 300", fifo.display());
        let line = serde_json::json!({"id": "b", "type": "bash", "command": command});
        child.send(format!("{line}\n").as_bytes());
        let shell = std::fs::read_to_string(&fifo).unwrap().trim().to_owned();
        assert!(!ended(&shell));
        child.signal(signal);
        let (status, errors) = child.wait_open();
        assert_eq!(status.code(), Some(code), "{signal}");
        assert_eq!(errors, "disposing\n", "{signal}");
        assert!(
            ended(&shell),
            "{signal}: the running shell command was killed"
        );
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
