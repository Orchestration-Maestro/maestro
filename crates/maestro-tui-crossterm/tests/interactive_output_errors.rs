//! Native output failures: a real standard-output pipe, the progress keepalive, the host's
//! standard error and a real input pseudo-terminal. Each scenario runs in a child process
//! that owns its standard descriptors.
#![cfg(unix)]

use std::cell::RefCell;
use std::io;
use std::rc::Rc;
use std::time::Duration;

use crate::runtime_support::{is_child, rerun, run_set};
use crate::support::{Rig, START_BYTES, elapse, pipe_output, pty, until};
use crate::{isolated, lifecycle_support::pty_input};
use maestro_tui::Terminal;
use maestro_tui::terminal::OutputErrorHandler;
use maestro_tui::tui::TuiRuntime;
use maestro_tui_crossterm::{ProcessTerminal, ProcessTuiRuntime};

/// A removal kept for the handler to run from inside its call.
type PendingRemoval = Rc<RefCell<Option<Box<dyn Fn()>>>>;

/// The failures one registered handler observed, and the removal it runs from inside its call.
#[derive(Clone, Default)]
struct Notices {
    /// The kinds of the failures it saw, in order.
    kinds: Rc<RefCell<Vec<io::ErrorKind>>>,
    /// The removal to run while the next call is still in progress.
    removal: PendingRemoval,
}

impl Notices {
    /// The handler to register.
    fn handler(&self) -> OutputErrorHandler {
        let seeing = self.clone();
        Rc::new(move |error: &io::Error| {
            seeing.kinds.borrow_mut().push(error.kind());
            let removal = seeing.removal.borrow_mut().take();
            if let Some(removal) = removal {
                removal();
            }
        })
    }

    /// Hands `removal` to the handler to run from inside its call.
    fn remove_during_call(&self, removal: Box<dyn Fn()>) {
        self.removal.replace(Some(removal));
    }

    /// How many failures it saw.
    fn count(&self) -> usize {
        self.kinds.borrow().len()
    }

    /// Waits until it saw `count` failures.
    async fn seen(&self, count: usize) {
        until(|| self.count() >= count).await;
    }
}

#[test]
fn synchronous_stdout_failure_notifies_once_without_reborrow() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let output = pipe_output();
        drop(output);
        let mut terminal = ProcessTerminal::new(rig.local());
        let notices = Notices::default();
        notices.remove_during_call(terminal.on_output_error(notices.handler()));
        let error = terminal
            .write("late output")
            .expect_err("the pipe has no reader");
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(notices.count(), 1);
    });
}

#[test]
fn asynchronous_stdout_failure_notifies_once() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let local = rig.local();
        let output = pipe_output();
        let notices = Notices::default();
        rig.run(async move {
            let mut terminal = ProcessTerminal::new(local);
            let removal = terminal.on_output_error(notices.handler());
            terminal
                .start(Box::new(|_| {}), Box::new(|| {}))
                .expect("the terminal starts");
            output.written(START_BYTES).await;
            drop(output);
            terminal.write("late output").expect("the writer takes it");
            notices.seen(1).await;
            removal();
            let _ = terminal.stop();
            assert_eq!(notices.count(), 1);
        });
    });
}

#[test]
fn progress_failure_notifies_once() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let local = rig.local();
        let output = pipe_output();
        let notices = Notices::default();
        rig.run(async move {
            let mut terminal = ProcessTerminal::new(local);
            let _removal = terminal.on_output_error(notices.handler());
            terminal.set_progress(true).expect("progress is shown");
            drop(output);
            elapse(1000).await;
            notices.seen(1).await;
            assert_eq!(notices.count(), 1);
        });
    });
}

#[test]
fn stderr_report_failure_notifies_once() {
    if is_child() {
        report_with_a_closed_standard_error();
        return;
    }
    let (reader, writer) = rustix::pipe::pipe().expect("a pipe opens");
    drop(reader);
    let output = rerun("stderr_report_failure_notifies_once", |process| {
        process.stderr(std::process::Stdio::from(writer))
    });
    assert!(
        output.stderr.is_empty(),
        "the closed pipe took the report: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Fails real scheduled work while standard error is the closed pipe, and counts the notices.
fn report_with_a_closed_standard_error() {
    run_set(true, |local| async move {
        let host = ProcessTuiRuntime::new(local);
        let notices = Notices::default();
        let _removal = host.on_output_error(notices.handler());
        drop(host.schedule(
            Duration::ZERO,
            Box::new(|| Err(io::Error::other("scheduled work failed"))),
        ));
        notices.seen(1).await;
        let seen = notices.kinds.borrow().clone();
        assert_eq!(seen, [io::ErrorKind::BrokenPipe]);
    });
}

/// Whether a read from a terminal whose controlling end closed fails with EIO here. Some
/// kernels report the end of input instead, which the reader treats as a normal end.
fn hangup_reads_as_an_error() -> bool {
    let (controlling, terminal) = pty();
    drop(controlling);
    let flags = rustix::fs::fcntl_getfl(&terminal).unwrap();
    rustix::fs::fcntl_setfl(&terminal, flags | rustix::fs::OFlags::NONBLOCK).unwrap();
    rustix::io::read(&terminal, &mut [0]) == Err(rustix::io::Errno::IO)
}

#[test]
fn native_stdin_eio_remains_an_input_failure() {
    isolated!(&[], || {
        if !hangup_reads_as_an_error() {
            eprintln!("this host reports a closed terminal as the end of input, not EIO");
            return;
        }
        let rig = Rig::paused();
        let input = pty_input();
        let _output = pipe_output();
        let notices = Notices::default();
        let local = rig.local();
        rig.run(async move {
            let mut terminal = ProcessTerminal::new(local);
            let _removal = terminal.on_output_error(notices.handler());
            terminal
                .start(Box::new(|_| {}), Box::new(|| {}))
                .expect("the terminal starts");
            input.close();
            elapse(1).await;
            let error = terminal.stop().expect_err("the input failure is retained");
            assert_eq!(
                error.raw_os_error(),
                Some(rustix::io::Errno::IO.raw_os_error()),
                "standard input failed with {error}"
            );
            assert_eq!(notices.count(), 0);
        });
    });
}
