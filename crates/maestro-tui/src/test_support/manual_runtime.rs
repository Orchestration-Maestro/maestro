//! A host whose clock and deferred callbacks a test controls.

use std::cell::RefCell;
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::rc::{Rc, Weak};
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use crate::tui::{LogContext, RenderCallback, RenderTimer, TuiRuntime};

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
    /// Futures handed over and not yet finished.
    futures: Vec<Local>,
}

/// Shared handle to the controlled host; clones observe the same world.
#[derive(Clone)]
pub struct ManualRuntime {
    /// State shared by every clone.
    state: Rc<RefCell<State>>,
    /// The agent directory [`log_context`](TuiRuntime::log_context) reports.
    agent_dir: PathBuf,
    /// The file name nonce [`log_context`](TuiRuntime::log_context) reports.
    nonce: String,
}

impl Default for ManualRuntime {
    fn default() -> Self {
        Self::new()
    }
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
#[must_use]
pub fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

impl ManualRuntime {
    /// Take submitted futures to await them in a controlled order.
    #[must_use]
    pub fn take_futures(&self) -> Vec<crate::tui::LocalFuture> {
        std::mem::take(&mut self.state.borrow_mut().futures)
    }

    /// Creates a host at time zero that reports the placeholder agent directory
    /// `/home/fixture`.
    #[must_use]
    pub fn new() -> Self {
        Self::with_agent_dir(PathBuf::from("/home/fixture"))
    }

    /// Creates a host at time zero whose diagnostic log files sit in `agent_dir`.
    ///
    /// The file name nonce is `agent_dir`'s own name, so runs sharing a log directory write
    /// separate records.
    #[must_use]
    pub fn with_agent_dir(agent_dir: PathBuf) -> Self {
        let nonce = nonce(&agent_dir);
        Self {
            state: Rc::new(RefCell::new(State::default())),
            agent_dir,
            nonce,
        }
    }

    /// The host as the writer receives it.
    #[must_use]
    pub fn handle(&self) -> Rc<dyn TuiRuntime> {
        Rc::new(self.clone())
    }

    /// The current time in whole milliseconds.
    #[must_use]
    pub fn millis(&self) -> u64 {
        u64::try_from(self.state.borrow().now.as_millis()).unwrap_or(u64::MAX)
    }

    /// Callbacks that have neither run nor been cancelled.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.state.borrow().pending.len()
    }

    /// Every pending deadline, in ascending order.
    #[must_use]
    pub fn deadlines(&self) -> Vec<Duration> {
        let mut deadlines: Vec<Duration> = self
            .state
            .borrow()
            .pending
            .iter()
            .map(|entry| entry.due)
            .collect();
        deadlines.sort_unstable();
        deadlines
    }

    /// Earliest pending deadline, if any.
    #[must_use]
    pub fn next_deadline(&self) -> Option<Duration> {
        self.state
            .borrow()
            .pending
            .iter()
            .map(|entry| entry.due)
            .min()
    }

    /// Moves the clock to `target` and runs every callback due by then, earliest first.
    ///
    /// Callbacks scheduled while running are included once they are due. Returns the
    /// result of each callback in the order it ran.
    #[must_use]
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
    #[must_use]
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
    #[must_use]
    pub fn futures(&self) -> usize {
        self.state.borrow().futures.len()
    }

    /// Polls every handed-over future, repeating while any finishes or new ones arrive.
    ///
    /// Finished futures are dropped; their errors are returned unchanged in completion order.
    #[must_use]
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

    fn log_context(&self) -> LogContext {
        LogContext {
            agent_dir: self.agent_dir.to_string_lossy().into_owned(),
            iso_time: "2026-01-02T03:04:05.006Z".to_owned(),
            unix_ms: 1_767_323_045_006,
            nonce: self.nonce.clone(),
        }
    }
}

/// The name of `agent_dir` in lowercase alphanumerics, which keeps concurrent records apart.
fn nonce(agent_dir: &Path) -> String {
    let name = agent_dir.file_name().map_or_else(String::new, |name| {
        name.to_string_lossy()
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .map(|character| character.to_ascii_lowercase())
            .collect()
    });
    if name.is_empty() {
        "i".to_owned()
    } else {
        name
    }
}
