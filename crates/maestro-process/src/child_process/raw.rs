//! Owned raw child resources driven by the existing collector and waiter.
use super::{CapturedChild, OutputStream, Termination, WaitOptions, wait};
use crate::byte_queue::Writes;
use std::{
    cell::{Cell, RefCell},
    future::Future,
    io,
    process::{ExitStatus, Stdio},
    rc::{Rc, Weak},
};
use tokio::{
    io::AsyncWriteExt,
    process::{ChildStdin, Command},
    sync::{Mutex, Notify},
};

/// Independent consuming output backed by the process owner's byte queue.
#[derive(Clone)]
pub struct RawOutput(Rc<Output>);
/// One descriptor's bytes, terminal observation and pending-read notification.
struct Output {
    /// Retained bytes and the descriptor's terminal state.
    state: RefCell<OutputState>,
    /// Wakes readers without introducing another output collector.
    changed: Notify,
}
/// Consuming state for one native descriptor.
#[derive(Default)]
struct OutputState {
    /// The shared native retained-front queue implementation.
    bytes: Writes,
    /// EOF or explicit consumer close, independently of child exit.
    closed: bool,
    /// A native failure follows any already delivered prefix.
    error: Option<io::Error>,
}
impl RawOutput {
    /// Constructs one open output consumer before collection starts.
    fn new() -> Self {
        Self(Rc::new(Output {
            state: RefCell::default(),
            changed: Notify::new(),
        }))
    }
    /// Reads available bytes, waiting without conflating pending input and EOF.
    ///
    /// # Errors
    /// Returns the native pipe failure after retained prefix bytes.
    pub async fn read(&self) -> io::Result<Option<Vec<u8>>> {
        loop {
            let changed = self.0.changed.notified();
            if let Some(result) = self.0.state.borrow_mut().ready() {
                return result;
            }
            changed.await;
        }
    }
    /// Releases queued bytes and this consumer without signalling its child.
    pub fn close(&self) {
        let mut state = self.0.state.borrow_mut();
        state.bytes = Writes::default();
        state.error = None;
        state.closed = true;
        drop(state);
        self.0.changed.notify_waiters();
    }
}
impl OutputState {
    /// Takes the next byte or terminal observation, leaving pending reads distinct.
    fn ready(&mut self) -> Option<io::Result<Option<Vec<u8>>>> {
        if let Some(bytes) = self.bytes.take_front() {
            return Some(Ok(Some(bytes)));
        }
        if let Some(error) = self.error.take() {
            return Some(Err(error));
        }
        self.closed.then_some(Ok(None))
    }
}
impl Output {
    /// Retains a new chunk only while its consumer remains open.
    fn data(&self, bytes: &[u8]) {
        let mut state = self.state.borrow_mut();
        if !state.closed {
            state.bytes.enqueue(bytes.to_vec());
        }
        drop(state);
        self.changed.notify_waiters();
    }
    /// Ends this descriptor without changing a previous close observation.
    fn finish(&self, error: Option<io::Error>) {
        let mut state = self.state.borrow_mut();
        if !state.closed {
            state.closed = true;
            state.error = error;
        }
        drop(state);
        self.changed.notify_waiters();
    }
}

/// Ordered native child input, shared by every retained input handle.
#[derive(Clone)]
pub struct RawInput(Rc<Mutex<Option<ChildStdin>>>);
impl RawInput {
    /// Writes the complete admitted byte sequence before the next input operation.
    ///
    /// # Errors
    /// Returns the original write failure or a closed-pipe error after EOF.
    pub async fn write(&self, bytes: &[u8]) -> io::Result<()> {
        let mut input = self.0.lock().await;
        let pipe = input
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "child stdin is closed"))?;
        pipe.write_all(bytes).await
    }
    /// Closes input after all earlier admitted writes.
    ///
    /// # Errors
    /// Returns the native shutdown failure.
    pub async fn end(&self) -> io::Result<()> {
        let mut input = self.0.lock().await;
        if let Some(mut pipe) = input.take() {
            pipe.shutdown().await?;
        }
        Ok(())
    }
}

/// Cloneable native failure retained for independent completion observers.
#[derive(Clone)]
struct Failure(io::ErrorKind, String);
impl From<io::Error> for Failure {
    fn from(error: io::Error) -> Self {
        Self(error.kind(), error.to_string())
    }
}
/// Exit and final output-close observations belong to one collector invocation.
struct Completion {
    /// The successful Windows request, whose process status does not carry a signal.
    #[cfg(windows)]
    requested_signal: RefCell<Option<String>>,
    /// Set at native exit, even when an inherited output descriptor stays open.
    exited: Cell<bool>,
    /// Filled only after the collector and native reaping have settled.
    result: RefCell<Option<Result<ExitStatus, Failure>>>,
    /// Wakes competing completion consumers.
    changed: Notify,
}

/// Numeric exit or signal termination observed by the retained process owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawExitStatus {
    /// Numeric exit, absent after a successful signal termination.
    code: Option<i32>,
    /// Actual Unix signal or successful Windows termination request.
    signal: Option<String>,
}
impl RawExitStatus {
    /// The numeric exit, absent for signal termination.
    #[must_use]
    pub fn code(&self) -> Option<i32> {
        self.code
    }
    /// The terminating native signal name, when known.
    #[must_use]
    pub fn signal(&self) -> Option<&str> {
        self.signal.as_deref()
    }
    /// Whether the child exited normally with code zero.
    #[must_use]
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

/// Retained child controls, with collection work scheduled by the caller.
#[derive(Clone)]
pub struct RawChild {
    /// Original native PID, not an identity selected from JSON.
    pid: Option<u32>,
    /// Selected native input, absent unless piped.
    pub stdin: Option<RawInput>,
    /// Selected output consumer, absent unless piped.
    pub stdout: Option<RawOutput>,
    /// Selected error consumer, absent unless piped.
    pub stderr: Option<RawOutput>,
    /// The sole waiter's observations.
    completion: Rc<Completion>,
}
impl RawChild {
    /// Launches with supplied descriptors and returns its sole collection future.
    ///
    /// The caller schedules that future on the same local executor as the controls.
    /// Dropping controls does not kill the child; collection continues to reap it.
    ///
    /// # Errors
    /// Returns the original native spawn or session-configuration failure.
    pub fn spawn(
        command: Command,
        stdio: [Stdio; 3],
        detached: bool,
    ) -> io::Result<(Self, impl Future<Output = ()> + 'static)> {
        let mut child = CapturedChild::spawn_with_stdio(command, stdio, detached)?;
        let owner = Self {
            pid: child.id(),
            stdin: child
                .take_stdin()
                .map(|pipe| RawInput(Rc::new(Mutex::new(Some(pipe))))),
            stdout: child.stdout.as_ref().map(|_| RawOutput::new()),
            stderr: child.stderr.as_ref().map(|_| RawOutput::new()),
            completion: Rc::new(Completion {
                #[cfg(windows)]
                requested_signal: RefCell::default(),
                exited: Cell::new(false),
                result: RefCell::default(),
                changed: Notify::new(),
            }),
        };
        let sink = Capture {
            stdout: owner.stdout.as_ref().map(|output| Rc::downgrade(&output.0)),
            stderr: owner.stderr.as_ref().map(|output| Rc::downgrade(&output.0)),
            completion: owner.completion.clone(),
        };
        Ok((owner, collect(child, sink)))
    }
    /// The admitted child's PID, retained after numeric exit.
    #[must_use]
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }
    /// Signals this child only before its native exit has been observed.
    ///
    /// # Errors
    /// Returns the original native signal failure.
    pub fn kill(&self, signal: Option<&str>) -> io::Result<bool> {
        if self.completion.exited.get() {
            return Ok(false);
        }
        if self
            .completion
            .result
            .borrow()
            .as_ref()
            .is_some_and(Result::is_err)
        {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "native child waiter is closed",
            ));
        }
        let Some(pid) = self.pid else {
            return Ok(false);
        };
        super::signal_pid(i32::try_from(pid).map_err(io::Error::other)?, signal)?;
        #[cfg(windows)]
        {
            *self.completion.requested_signal.borrow_mut() =
                Some(signal.unwrap_or("SIGTERM").to_owned());
        }
        Ok(true)
    }
    /// Observes final status without draining or replaying either output consumer.
    ///
    /// # Errors
    /// Returns the native collection or wait failure.
    pub async fn wait(&self) -> io::Result<RawExitStatus> {
        loop {
            let changed = self.completion.changed.notified();
            if let Some(result) = self.completion.result.borrow().clone() {
                let status =
                    result.map_err(|Failure(kind, message)| io::Error::new(kind, message))?;
                #[cfg(windows)]
                let signal = self.completion.requested_signal.borrow().clone();
                #[cfg(not(windows))]
                let signal = super::exit_signal_name(status).map(str::to_owned);
                let code = signal.is_none().then(|| status.code()).flatten();
                return Ok(RawExitStatus { code, signal });
            }
            changed.await;
        }
    }
}

/// Descriptor-tagged delivery from the original native capture loop.
struct Capture {
    /// Weak consumers let the last guest handle release queued output immediately.
    stdout: Option<Weak<Output>>,
    /// Error output has its own byte and close observations.
    stderr: Option<Weak<Output>>,
    /// The original waiter publishes exit before completing inherited pipe collection.
    completion: Rc<Completion>,
}
impl Capture {
    /// Selects the original live consumer without retaining an absent one.
    fn output(&self, stream: OutputStream) -> Option<Rc<Output>> {
        match stream {
            OutputStream::Stdout => &self.stdout,
            OutputStream::Stderr => &self.stderr,
        }
        .as_ref()
        .and_then(Weak::upgrade)
    }
}
impl wait::Observer for Capture {
    fn data(&mut self, stream: OutputStream, bytes: &[u8]) {
        if let Some(output) = self.output(stream) {
            output.data(bytes);
        }
    }
    fn closed(&mut self, stream: OutputStream) {
        if let Some(output) = self.output(stream) {
            output.finish(None);
        }
    }
    fn failed(&mut self, stream: OutputStream, error: &io::Error) -> bool {
        if let Some(output) = self.output(stream) {
            output.finish(Some(io::Error::new(error.kind(), error.to_string())));
        }
        true
    }
    fn exited(&mut self, _status: ExitStatus) {
        self.completion.exited.set(true);
    }
}
/// Runs exactly one existing collector and preserves per-descriptor terminal errors.
async fn collect(mut child: CapturedChild, mut sink: Capture) {
    let result = wait::wait(
        &mut child,
        WaitOptions::default(),
        std::future::pending::<Termination>(),
        Some(&mut sink),
    )
    .await;
    let result = result.map(|output| output.status).map_err(Failure::from);
    for output in [&sink.stdout, &sink.stderr]
        .into_iter()
        .flatten()
        .filter_map(Weak::upgrade)
    {
        let error = result
            .as_ref()
            .err()
            .map(|Failure(kind, text)| io::Error::new(*kind, text.clone()));
        output.finish(error);
    }
    if result.is_ok() {
        sink.completion.exited.set(true);
    }
    *sink.completion.result.borrow_mut() = Some(result);
    sink.completion.changed.notify_waiters();
}

#[cfg(test)]
mod tests;
