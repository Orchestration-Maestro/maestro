//! A host whose clock, deferred callbacks, environment and log files a test controls.

use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::rc::{Rc, Weak};
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use maestro_tui::tui::{LogContext, RenderCallback, RenderTimer, TuiRuntime};

/// A log file effect the writer asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileEffect {
    /// Text appended to an existing file.
    Append(PathBuf, String),
    /// A file replaced after its directory was created.
    Write(PathBuf, String),
}

/// A callback waiting for its due time.
struct Pending {
    /// Identifies the callback for cancellation and ordering.
    id: u64,
    /// The time the callback becomes due.
    due: Duration,
    /// The work to run.
    callback: RenderCallback,
}

/// A future the editor handed to the host.
type Local = Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error>>>>>;

/// The controlled world.
#[derive(Default)]
struct State {
    /// The current monotonic time.
    now: Duration,
    /// The identifier the next callback receives.
    next_id: u64,
    /// Callbacks that have not run or been cancelled.
    pending: Vec<Pending>,
    /// Environment variables that are set.
    environment: HashMap<String, String>,
    /// Log effects in the order requested.
    files: Vec<FileEffect>,
    /// Futures handed over and not yet finished.
    futures: Vec<Local>,
}

/// Shared handle to the controlled host; clones observe the same world.
#[derive(Clone, Default)]
pub struct ManualRuntime {
    /// State shared by every clone.
    state: Rc<RefCell<State>>,
}

/// Cancels one callback of a [`ManualRuntime`].
struct ManualTimer {
    /// The world the callback waits in.
    state: Weak<RefCell<State>>,
    /// The callback to cancel.
    id: u64,
}

impl RenderTimer for ManualTimer {
    fn cancel(&mut self) {
        if let Some(state) = self.state.upgrade() {
            state
                .borrow_mut()
                .pending
                .retain(|entry| entry.id != self.id);
        }
    }
}

/// Converts whole milliseconds to a duration.
pub fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

impl ManualRuntime {
    /// Creates a host at time zero with an empty environment.
    pub fn new() -> Self {
        Self::default()
    }

    /// The host as the writer receives it.
    pub fn handle(&self) -> Rc<dyn TuiRuntime> {
        Rc::new(self.clone())
    }

    /// The current time in whole milliseconds.
    pub fn millis(&self) -> u64 {
        u64::try_from(self.state.borrow().now.as_millis()).unwrap_or(u64::MAX)
    }

    /// Sets an environment variable.
    pub fn set_environment(&self, key: &str, value: &str) {
        self.state
            .borrow_mut()
            .environment
            .insert(key.to_owned(), value.to_owned());
    }

    /// Callbacks that have neither run nor been cancelled.
    pub fn pending(&self) -> usize {
        self.state.borrow().pending.len()
    }

    /// Earliest pending deadline, if any.
    pub fn next_deadline(&self) -> Option<Duration> {
        self.state
            .borrow()
            .pending
            .iter()
            .map(|entry| entry.due)
            .min()
    }

    /// Log effects in the order requested.
    pub fn files(&self) -> Vec<FileEffect> {
        self.state.borrow().files.clone()
    }

    /// Moves the clock to `target` and runs every callback due by then, earliest first.
    ///
    /// Callbacks scheduled while running are included once they are due. Returns the
    /// result of each callback in the order it ran.
    pub fn advance_to(&self, target: Duration) -> Vec<io::Result<()>> {
        {
            let mut state = self.state.borrow_mut();
            state.now = state.now.max(target);
        }
        let mut results = Vec::new();
        while let Some(entry) = self.take_due() {
            results.push((entry.callback)());
        }
        results
    }

    /// Runs the callbacks that are due at the current time.
    pub fn run_due(&self) -> Vec<io::Result<()>> {
        self.advance_to(self.now())
    }

    /// Runs callbacks, advancing the clock to each due time, until none remain.
    ///
    /// # Errors
    ///
    /// Returns the first error among the callbacks due at one moment; all of them run
    /// first, and callbacks due later stay pending.
    pub fn settle(&self) -> io::Result<()> {
        loop {
            let next = self
                .state
                .borrow()
                .pending
                .iter()
                .map(|entry| entry.due)
                .min();
            let Some(due) = next else { return Ok(()) };
            for result in self.advance_to(due) {
                result?;
            }
        }
    }

    /// Futures handed over that have not finished.
    pub fn futures(&self) -> usize {
        self.state.borrow().futures.len()
    }

    /// Polls every handed-over future, repeating while any finishes or new ones arrive.
    ///
    /// Finished futures are dropped; their errors are returned unchanged in completion order.
    pub fn drain_futures(&self) -> Vec<Box<dyn std::error::Error>> {
        let mut errors = Vec::new();
        let mut context = Context::from_waker(Waker::noop());
        loop {
            let batch = std::mem::take(&mut self.state.borrow_mut().futures);
            let before = batch.len();
            let mut pending = Vec::new();
            for mut future in batch {
                match future.as_mut().poll(&mut context) {
                    Poll::Pending => pending.push(future),
                    Poll::Ready(result) => errors.extend(result.err()),
                }
            }
            let progressed = pending.len() < before;
            let mut state = self.state.borrow_mut();
            let spawned = !state.futures.is_empty();
            pending.append(&mut state.futures);
            state.futures = pending;
            if !progressed && !spawned {
                return errors;
            }
        }
    }

    /// Removes and returns the earliest callback that is due.
    fn take_due(&self) -> Option<Pending> {
        let mut state = self.state.borrow_mut();
        let now = state.now;
        let index = state
            .pending
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.due <= now)
            .min_by_key(|(_, entry)| (entry.due, entry.id))
            .map(|(index, _)| index)?;
        Some(state.pending.remove(index))
    }
}

impl TuiRuntime for ManualRuntime {
    fn now(&self) -> Duration {
        self.state.borrow().now
    }

    fn schedule(&self, delay: Duration, callback: RenderCallback) -> Box<dyn RenderTimer> {
        let mut state = self.state.borrow_mut();
        let id = state.next_id;
        state.next_id += 1;
        let due = state.now + delay;
        state.pending.push(Pending { id, due, callback });
        Box::new(ManualTimer {
            state: Rc::downgrade(&self.state),
            id,
        })
    }

    fn spawn_local(&self, future: Local) {
        self.state.borrow_mut().futures.push(future);
    }

    fn environment(&self, key: &str) -> Option<String> {
        self.state.borrow().environment.get(key).cloned()
    }

    fn log_context(&self) -> LogContext {
        LogContext {
            home: PathBuf::from("/home/fixture"),
            iso_time: "2026-01-02T03:04:05.006Z".to_owned(),
            unix_ms: 1_767_323_045_006,
            nonce: "i".to_owned(),
        }
    }

    fn append_log(&self, path: &Path, contents: &str) -> io::Result<()> {
        let effect = FileEffect::Append(path.to_path_buf(), contents.to_owned());
        self.state.borrow_mut().files.push(effect);
        Ok(())
    }

    fn write_log(&self, path: &Path, contents: &str) -> io::Result<()> {
        let effect = FileEffect::Write(path.to_path_buf(), contents.to_owned());
        self.state.borrow_mut().files.push(effect);
        Ok(())
    }
}
