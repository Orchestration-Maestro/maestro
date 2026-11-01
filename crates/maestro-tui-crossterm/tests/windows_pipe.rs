//! Redirected Windows standard input, including a pipe kept open during shutdown.
#![cfg(test)]
#![cfg(windows)]

use std::cell::RefCell;
use std::future::Future;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::pin::pin;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::task::{Context, Waker};
use std::time::Duration;

use maestro_test_process::TestProcess;
use maestro_tui::Terminal;
use maestro_tui_crossterm::ProcessTerminal;
use maestro_tui_crossterm::terminal::wait_for_input;
use tokio::runtime::Builder;
use tokio::task::LocalSet;

/// Marks the copy whose standard input is the pipe under test.
const ROLE: &str = "MAESTRO_TERMINAL_TEST_PIPE";

/// The child has installed its reader, or has stopped it for the restart scenario.
const READY: &str = "pipe ready";

/// Runs a scenario in a child with supplied bytes and optional EOF on standard input.
fn with_pipe(test: &str, bytes: &[u8], eof: bool, scenario: impl FnOnce()) {
    if std::env::var_os(ROLE).is_some() {
        scenario();
        return;
    }
    TestProcess::current(test)
        .unwrap()
        .nocapture()
        .single_threaded()
        .invoke_with(|arguments| -> io::Result<()> {
            let mut child = Command::new(std::env::current_exe()?)
                .args(arguments)
                .env(ROLE, "1")
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()?;
            let mut input = child.stdin.take();
            let mut output = BufReader::new(child.stderr.take().unwrap());
            let mut line = String::new();
            loop {
                assert_ne!(
                    output.read_line(&mut line)?,
                    0,
                    "child ended before readiness: {line}"
                );
                if line.trim_end() == READY {
                    break;
                }
                line.clear();
            }
            input.as_mut().unwrap().write_all(bytes)?;
            if eof {
                drop(input.take());
            }
            let mut report = String::new();
            output.read_to_string(&mut report)?;
            let status = child.wait()?;
            assert!(status.success(), "pipe scenario failed: {report}");
            Ok(())
        })
        .unwrap();
}

/// Drives the terminal on a caller-owned local runtime.
fn drive<F: Future<Output = ()>>(scenario: impl FnOnce(Rc<LocalSet>) -> F) {
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    let local = Rc::new(LocalSet::new());
    runtime.block_on(local.run_until(scenario(Rc::clone(&local))));
}

/// Starts input collection on an existing terminal.
fn collect(terminal: &mut ProcessTerminal) -> Rc<RefCell<String>> {
    let text = Rc::new(RefCell::new(String::new()));
    let seen = Rc::clone(&text);
    terminal
        .start(
            Box::new(move |chunk| seen.borrow_mut().push_str(chunk)),
            Box::new(|| {}),
        )
        .unwrap();
    text
}

/// Waits for the callback's completion observation, not a fixed delay.
async fn until(done: impl Fn() -> bool) {
    while !done() {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[test]
fn redirected_input_keeps_nul_unicode_paste_and_keyboard_negotiation() {
    with_pipe(
        "redirected_input_keeps_nul_unicode_paste_and_keyboard_negotiation",
        "\x1b[?1u\0é\x1b[200~paste\x1b[201~\x1b[Z".as_bytes(),
        true,
        || {
            drive(|local| async move {
                let mut terminal = ProcessTerminal::new(local);
                let text = collect(&mut terminal);
                eprintln!("{READY}");
                let expected = "\0é\x1b[200~paste\x1b[201~\x1b[Z";
                until(|| text.borrow().len() >= expected.len()).await;
                assert_eq!(&*text.borrow(), expected);
                assert!(terminal.kitty_protocol_active());
                terminal.stop().unwrap();
                assert!(!terminal.kitty_protocol_active());
            });
        },
    );
}

#[test]
fn stop_and_restart_with_a_pending_pipe_read_delivers_later_input_once() {
    with_pipe(
        "stop_and_restart_with_a_pending_pipe_read_delivers_later_input_once",
        b"resumed",
        false,
        || {
            drive(|local| async move {
                let mut terminal = ProcessTerminal::new(local);
                let old = collect(&mut terminal);
                tokio::task::yield_now().await;
                terminal.stop().unwrap();
                eprintln!("{READY}");
                let text = collect(&mut terminal);
                until(|| text.borrow().len() >= 7).await;
                assert_eq!(&*text.borrow(), "resumed");
                assert!(old.borrow().is_empty());
                terminal.stop().unwrap();
            });
        },
    );
}

#[test]
fn standalone_wait_and_started_terminal_share_redirected_input() {
    with_pipe(
        "standalone_wait_and_started_terminal_share_redirected_input",
        b"x",
        false,
        || {
            drive(|local| async move {
                let mut terminal = ProcessTerminal::new(local);
                let text = collect(&mut terminal);
                let mut wait = pin!(wait_for_input());
                assert!(
                    wait.as_mut()
                        .poll(&mut Context::from_waker(Waker::noop()))
                        .is_pending()
                );
                eprintln!("{READY}");
                wait.await.unwrap();
                until(|| !text.borrow().is_empty()).await;
                assert_eq!(&*text.borrow(), "x");
                terminal.stop().unwrap();
            });
        },
    );
}

#[test]
fn standalone_wait_on_an_idle_pipe_can_be_dropped() {
    with_pipe(
        "standalone_wait_on_an_idle_pipe_can_be_dropped",
        b"",
        false,
        || {
            drive(|local| async move {
                let mut terminal = ProcessTerminal::new(local);
                let text = collect(&mut terminal);
                eprintln!("{READY}");
                let mut wait = pin!(wait_for_input());
                assert!(
                    wait.as_mut()
                        .poll(&mut Context::from_waker(Waker::noop()))
                        .is_pending()
                );
                terminal.stop().unwrap();
                assert!(text.borrow().is_empty());
            });
        },
    );
}
