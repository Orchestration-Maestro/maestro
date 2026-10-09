//! Child-process scenarios and descriptor rigs for the process terminal tests.
//!
//! `ProcessTerminal` owns the standard descriptors of its process, so every scenario runs in
//! a child copy of its test with fresh descriptors installed over 0 and 1. The scenario
//! drives a current-thread runtime whose clock is either paused (deadline decisions) or
//! real (the reactor waking for a real deadline).
//!
//! Observations wait on the operation that produces them: a callback that ran, bytes that
//! reached a descriptor, a descriptor that was released, or the paused clock reaching a time.
//! The runtime moves a paused clock only while no task can run, so every task whose deadline
//! lies before the time reached has run by then.
#![cfg(test)]

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::os::fd::OwnedFd;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::Duration;

use rustix::event::{PollFd, PollFlags, poll};
use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use rustix::io::{Errno, read, write};
use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
use rustix::stdio::{dup2_stdin, dup2_stdout, stdin, stdout};
use rustix::termios::{Winsize, tcsetwinsize};
use tokio::runtime::{Builder, Runtime};
use tokio::task::LocalSet;

/// Marks the child copy of a test, which runs the scenario instead of spawning itself.
const CHILD_MARKER: &str = "MAESTRO_TERMINAL_TEST_CHILD";

/// Names which scenario of a test the child copy runs.
const CASE_MARKER: &str = "MAESTRO_TERMINAL_TEST_CASE";

/// Variables a scenario sees only when it sets them.
const SCENARIO_VARIABLES: [&str; 4] = ["COLUMNS", "LINES", "MAESTRO_TUI_WRITE_LOG", "TZ"];

/// The bytes a terminal writes when it starts: bracketed paste on, then the keyboard query.
pub const START_BYTES: &str = "\x1b[?2004h\x1b[?u";

/// Runs scenario number `case` of `test` in a child copy of the calling test and fails the
/// test when the child fails. Inside the child it runs `scenario` only when it is the case
/// the parent chose. `environment` is passed to the child on top of a clean slate.
pub fn isolated(test: &str, case: usize, environment: &[(&str, &str)], scenario: impl FnOnce()) {
    if is_child() {
        if std::env::var(CASE_MARKER).is_ok_and(|chosen| chosen == case.to_string()) {
            scenario();
        }
        return;
    }
    let mut command =
        Command::new(std::env::current_exe().expect("the test executable has a path"));
    for name in SCENARIO_VARIABLES {
        command.env_remove(name);
    }
    let output = command
        .args([test, "--exact", "--test-threads=1", "--nocapture"])
        .env(CHILD_MARKER, "1")
        .env(CASE_MARKER, case.to_string())
        .envs(environment.iter().copied())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the child test starts")
        .wait_with_output()
        .expect("the child output is collected");
    assert!(
        output.status.success(),
        "{test} case {case} failed in its child:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Whether this process is the child copy of a test.
pub fn is_child() -> bool {
    std::env::var_os(CHILD_MARKER).is_some()
}

/// Names the calling test from a marker function declared inside it. A first argument
/// numbers the scenario when a test runs several.
#[macro_export]
macro_rules! isolated {
    ($environment:expr, $scenario:expr) => {
        $crate::isolated!(0, $environment, $scenario)
    };
    ($case:expr, $environment:expr, $scenario:expr) => {{
        fn marker() {}
        $crate::support::isolated(
            $crate::support::test_name(::std::any::type_name_of_val(&marker)),
            $case,
            $environment,
            $scenario,
        )
    }};
}

/// The test function name inside the path of its marker function.
pub fn test_name(marker: &str) -> &str {
    let function = marker.strip_suffix("::marker").expect("a marker function");
    function.rsplit("::").next().expect("a path has a segment")
}

/// The tools one scenario uses: the runtime, the local task set and the replaced descriptors.
pub struct Rig {
    /// The current-thread runtime that drives the scenario.
    runtime: Runtime,
    /// The set the terminal under test spawns its tasks on.
    local: Rc<LocalSet>,
    /// The standard input and output the process started with, put back on drop.
    originals: (OwnedFd, OwnedFd),
}

impl Rig {
    /// A rig whose clock the runtime moves only while no task can run, to the earliest deadline.
    pub fn paused() -> Self {
        Self::on(
            Builder::new_current_thread()
                .enable_all()
                .start_paused(true),
        )
    }

    /// Builds the rig around `builder` and remembers the original descriptors.
    pub fn on(builder: &mut Builder) -> Self {
        Self {
            runtime: builder.build().expect("the runtime builds"),
            local: Rc::new(LocalSet::new()),
            originals: (
                rustix::io::dup(stdin()).expect("stdin duplicates"),
                rustix::io::dup(stdout()).expect("stdout duplicates"),
            ),
        }
    }

    /// The local task set to give the terminal.
    pub fn local(&self) -> Rc<LocalSet> {
        Rc::clone(&self.local)
    }

    /// Runs `scenario` on the runtime, with the local set running beside it.
    pub fn run<T>(&self, scenario: impl Future<Output = T>) -> T {
        self.runtime.block_on(self.local.run_until(scenario))
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = dup2_stdin(&self.originals.0);
        let _ = dup2_stdout(&self.originals.1);
    }
}

/// Installs a pipe as standard input and returns its writing end.
pub fn pipe_input() -> Feed {
    let (reader, writer) = rustix::pipe::pipe().expect("a pipe opens");
    dup2_stdin(&reader).expect("the pipe becomes standard input");
    Feed(writer)
}

/// Installs a pipe as standard output and returns what is written to it.
pub fn pipe_output() -> Capture {
    let (reader, writer) = rustix::pipe::pipe().expect("a pipe opens");
    dup2_stdout(&writer).expect("the pipe becomes standard output");
    Capture::new(reader)
}

/// Installs a pseudo-terminal as standard output and returns what is written to it.
pub fn pty_output() -> Capture {
    let (master, slave) = pty();
    dup2_stdout(&slave).expect("the slave becomes standard output");
    Capture::new(master)
}

/// The scenario's end of standard input.
pub struct Feed(pub OwnedFd);

impl Feed {
    /// Writes `bytes` and returns once standard input holds them.
    pub fn send(&self, bytes: &[u8]) {
        let mut rest = bytes;
        while !rest.is_empty() {
            let written = write(&self.0, rest).expect("the input accepts the bytes");
            rest = &rest[written..];
        }
        let stdin = stdin();
        let mut fds = [PollFd::new(&stdin, PollFlags::IN)];
        while poll(&mut fds, None) == Err(Errno::INTR) {}
    }
}

/// The scenario's end of standard output.
pub struct Capture(pub OwnedFd);

impl Capture {
    /// Wraps the reading end and makes its reads nonblocking.
    fn new(reader: OwnedFd) -> Self {
        let flags = fcntl_getfl(&reader).expect("flags read");
        fcntl_setfl(&reader, flags | OFlags::NONBLOCK).expect("flags write");
        Self(reader)
    }

    /// Everything written since the last call.
    pub fn take(&self) -> String {
        let mut bytes = Vec::new();
        let mut chunk = [0; 256];
        while let Ok(count @ 1..) = read(&self.0, &mut chunk) {
            bytes.extend_from_slice(&chunk[..count]);
        }
        String::from_utf8(bytes).expect("terminal output is text")
    }

    /// Waits until `expected` has been written since the last observation, then checks that
    /// nothing else came with it.
    pub async fn written(&self, expected: &str) {
        let mut collected = String::new();
        until(|| {
            collected.push_str(&self.take());
            collected.len() >= expected.len()
        })
        .await;
        assert_eq!(collected, expected);
    }

    /// Sets the window size of the pseudo-terminal this capture reads.
    pub fn resize(&self, columns: u16, rows: u16) {
        set_size(&self.0, columns, rows);
    }
}

/// Opens a pseudo-terminal pair: the controlling end and the terminal end.
pub fn pty() -> (OwnedFd, OwnedFd) {
    let master = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).expect("a pty opens");
    grantpt(&master).expect("the pty is granted");
    unlockpt(&master).expect("the pty unlocks");
    let name = ptsname(&master, Vec::new()).expect("the pty has a name");
    let slave = rustix::fs::open(
        name.as_c_str(),
        OFlags::RDWR | OFlags::NOCTTY,
        rustix::fs::Mode::empty(),
    )
    .expect("the terminal end opens");
    (master, slave)
}

/// Gives the pseudo-terminal behind `fd` a window size.
pub fn set_size(fd: &OwnedFd, columns: u16, rows: u16) {
    let size = Winsize {
        ws_row: rows,
        ws_col: columns,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    tcsetwinsize(fd, size).expect("the size is set");
}

/// Yields to the runtime until `done` holds. The condition is the effect being waited for,
/// so the wait ends exactly when the producing operation has finished.
pub async fn until(mut done: impl FnMut() -> bool) {
    while !done() {
        tokio::task::yield_now().await;
    }
}

/// Moves the paused clock forward by `milliseconds`. The runtime stops the clock at every
/// earlier deadline and runs the tasks it wakes before moving on, so an effect due strictly
/// before the time reached has happened when this returns. A deadline at the time reached
/// wakes its task together with the caller, so such an effect is waited for with [`until`].
pub async fn elapse(milliseconds: u64) {
    tokio::time::sleep(Duration::from_millis(milliseconds)).await;
}

/// Input chunks a terminal delivered, in order.
#[derive(Clone, Default)]
pub struct Inputs(pub Rc<RefCell<Vec<String>>>);

impl Inputs {
    /// The callback to give a terminal.
    pub fn callback(&self) -> Box<dyn FnMut(&str)> {
        let chunks = Rc::clone(&self.0);
        Box::new(move |chunk| chunks.borrow_mut().push(chunk.to_owned()))
    }
}

/// Window-size notices a terminal delivered.
#[derive(Clone, Default)]
pub struct Resizes(pub Rc<Cell<usize>>);

impl Resizes {
    /// The callback to give a terminal.
    pub fn callback(&self) -> Box<dyn FnMut()> {
        let count = Rc::clone(&self.0);
        Box::new(move || count.set(count.get() + 1))
    }
}
