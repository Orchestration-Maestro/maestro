//! Ordered terminal actor records and native control operations.
use super::controls::{Admission, Control, Pending, Reply};
use super::{controls, runner};
use crate::pty::PtySize;
use std::{
    future::Future,
    io,
    os::fd::OwnedFd,
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    sync::mpsc::{
        self,
        error::{SendError, TrySendError},
    },
    task::JoinHandle,
};

/// Ordered terminal replies produced by one read callback.
#[derive(Default)]
pub struct PtyReadResult {
    /// Bytes written back to the terminal in order.
    pub terminal_responses: Vec<Vec<u8>>,
}

/// Byte-oriented callback owned by the actor.
pub(crate) type ReadCallback = Box<dyn FnMut(&[u8]) -> PtyReadResult + Send + 'static>;
/// One-shot factory for an owned asynchronous reader-exit notification.
pub(crate) type ReaderExitCallback =
    Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> + Send + 'static>;

/// Owned descriptor, initial state and callbacks for one I/O actor.
pub struct PtyIoActorConfig {
    /// Identity attached to structured diagnostics.
    pub pane_id: u32,
    /// Terminal master moved into the worker.
    pub master_fd: OwnedFd,
    /// Whether user admission and terminal reads start paused.
    pub initially_quiesced: bool,
    /// Receives terminal bytes and returns ordered replies.
    pub on_read: ReadCallback,
    /// Invoked and awaited on normal worker exit before its resources are released.
    ///
    /// Graceful shutdown must await pending hooks before dropping the Tokio runtime.
    /// Destroying that runtime cancels a pending hook and releases the master.
    pub on_reader_exit: Option<ReaderExitCallback>,
}

/// Factory for one terminal I/O worker on the current Tokio runtime.
pub struct PtyIoActor;

/// Shared handle lifetime; the worker holds no strong reference to this owner.
pub(super) struct Owner {
    /// User-admission state shared with the worker.
    pub(super) admission: Arc<Admission>,
    /// Bounded user-input channel.
    pub(super) data: mpsc::Sender<Vec<u8>>,
    /// Controls independent of bounded input capacity.
    pub(super) control: mpsc::UnboundedSender<controls::Control>,
    /// Coalesced geometry and ordered callback responses.
    pub(super) controls: Arc<controls::Controls>,
    /// Retained completion owner.
    pub(super) task: Mutex<Option<JoinHandle<()>>>,
}

impl Drop for Owner {
    fn drop(&mut self) {
        self.admission.close();
        let _ = self.control.send(controls::Control::Shutdown);
    }
}

/// Cloneable input/control handle; dropping the last handle requests worker exit.
#[derive(Clone)]
pub struct PtyIoActorHandle {
    /// Shared completion and submission ownership.
    pub(super) owner: Arc<Owner>,
}

impl PtyIoActor {
    /// Enables close-on-exec and nonblocking on the master before starting its worker.
    ///
    /// # Errors
    /// Returns descriptor configuration or runtime registration failures.
    pub fn spawn(config: PtyIoActorConfig) -> io::Result<PtyIoActorHandle> {
        let runtime = tokio::runtime::Handle::try_current().map_err(io::Error::other)?;
        let flags = rustix::io::fcntl_getfd(&config.master_fd)?;
        rustix::io::fcntl_setfd(&config.master_fd, flags | rustix::io::FdFlags::CLOEXEC)?;
        crate::pty::fd::set_nonblocking(&config.master_fd)?;
        let (data, data_rx) = mpsc::channel(1024);
        let (control, control_rx) = mpsc::unbounded_channel();
        let admission = Arc::new(Admission::new(!config.initially_quiesced));
        let controls = Arc::new(controls::Controls::default());
        let runner = runner::Runner::new(
            config,
            data_rx,
            control_rx,
            Arc::clone(&admission),
            Arc::clone(&controls),
        )?;
        let task = runtime.spawn(runner.run());
        Ok(PtyIoActorHandle {
            owner: Arc::new(Owner {
                admission,
                data,
                control,
                controls,
                task: Mutex::new(Some(task)),
            }),
        })
    }
}

impl PtyIoActorHandle {
    /// Waits for input capacity, checking admission again after that wait.
    ///
    /// # Errors
    /// Returns the original bytes when admission or the worker is closed.
    pub async fn write_user_input(&self, bytes: Vec<u8>) -> Result<(), SendError<Vec<u8>>> {
        if self.owner.admission.closed() {
            return Err(SendError(bytes));
        }
        let Ok(permit) = self.owner.data.reserve().await else {
            return Err(SendError(bytes));
        };
        let Some(_admitted) = self.owner.admission.enter() else {
            return Err(SendError(bytes));
        };
        permit.send(bytes);
        Ok(())
    }

    /// Submits input without waiting, preserving Full versus Closed and the bytes.
    ///
    /// # Errors
    /// Returns a full-queue or closed-admission error with the original bytes.
    pub fn try_write_user_input(&self, bytes: Vec<u8>) -> Result<(), TrySendError<Vec<u8>>> {
        let Some(_admitted) = self.owner.admission.enter() else {
            return Err(TrySendError::Closed(bytes));
        };
        self.owner.data.try_send(bytes)
    }

    /// Closes user admission and requests worker exit.
    pub fn shutdown(&self) {
        self.owner.admission.close();
        let _ = self.owner.control.send(controls::Control::Shutdown);
    }
}

impl PtyIoActorHandle {
    /// Runs the response producer synchronously in the same ordered lane as reads.
    pub fn write_terminal_response(&self, response: impl FnOnce() -> Option<Vec<u8>>) {
        let controls = &self.owner.controls;
        let _lease = controls.order.enter();
        if let Some(bytes) = response().filter(|bytes| !bytes.is_empty()) {
            controls
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .responses
                .push(bytes);
            controls.changed.notify_one();
        }
    }

    /// Coalesces the latest size and replies, independently of user-input capacity.
    pub fn resize(&self, size: PtySize, terminal_responses: Vec<Vec<u8>>) {
        let old = self
            .owner
            .controls
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .resize
            .replace((size, terminal_responses));
        drop(old);
        self.owner.controls.changed.notify_one();
    }

    /// Coalesces a redraw nudge followed by restoration of the supplied size.
    pub fn nudge_child_redraw_after_handoff(&self, size: PtySize) {
        self.owner
            .controls
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .nudge = Some(size);
        self.owner.controls.changed.notify_one();
    }
}

impl super::runner::Runner {
    /// Applies geometry before its replies and preserves ordinary response order.
    pub(super) async fn apply_controls(&mut self) {
        let pending = std::mem::take(
            &mut *self
                .controls
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        let Pending {
            resize,
            nudge,
            responses,
        } = pending;
        if let Some((size, responses)) = resize {
            self.resize(size);
            for bytes in responses {
                self.enqueue(bytes);
            }
        }
        if let Some(size) = nudge {
            let temporary = if size.rows > 2 {
                PtySize {
                    rows: size.rows - 1,
                    ..size
                }
            } else {
                PtySize {
                    cols: size.cols.saturating_sub(1).max(4),
                    ..size
                }
            };
            if temporary != size {
                self.resize(temporary);
                tokio::time::sleep(std::time::Duration::from_millis(30)).await;
                self.resize(size);
            }
        }
        for bytes in responses {
            self.enqueue(bytes);
        }
    }

    /// Emits the native resize diagnostic without changing reply order.
    fn resize(&self, size: PtySize) {
        if let Err(error) = crate::pty::fd::resize_pty_fd(self.fd.get_ref(), size) {
            tracing::debug!(pane=self.pane,err=%error,"PTY resize failed");
        }
    }
}

/// Constructs a closed-worker error without changing native I/O errors.
fn closed() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "pty actor closed")
}

impl PtyIoActorHandle {
    /// Closes input admission and waits for accepted writes to drain and reads to pause.
    ///
    /// # Errors
    /// Returns the supplied-budget, drain, descriptor or closed-worker error.
    pub async fn begin_handoff(&self, timeout: Duration) -> io::Result<()> {
        self.owner.admission.close();
        let result = self
            .request(
                Control::Begin,
                timeout,
                "timed out waiting for PTY actor to quiesce",
            )
            .await;
        if result.is_err() {
            let _ = self.rollback_handoff().await;
        }
        result
    }

    /// Duplicates the master only after successful quiescence.
    ///
    /// # Errors
    /// Returns a state, duplicate, closed-worker or one-second reply error.
    pub async fn duplicate_for_handoff(&self) -> io::Result<OwnedFd> {
        self.request(
            Control::Duplicate,
            Duration::from_secs(1),
            "timed out waiting for PTY handoff duplicate",
        )
        .await
    }

    /// Queries the master foreground group within the one-second reply budget.
    pub async fn foreground_process_group_id(&self) -> Option<u32> {
        let (reply, receiver) = tokio::sync::oneshot::channel();
        self.owner.control.send(Control::Foreground(reply)).ok()?;
        tokio::time::timeout(Duration::from_secs(1), receiver)
            .await
            .ok()?
            .ok()?
    }

    /// Resumes reads and reopens user admission after a successful acknowledgement.
    ///
    /// # Errors
    /// Returns a closed-worker or one-second reply error.
    pub async fn rollback_handoff(&self) -> io::Result<()> {
        self.request(
            Control::Rollback,
            Duration::from_secs(1),
            "timed out waiting for PTY handoff rollback",
        )
        .await?;
        self.owner.admission.reopen();
        Ok(())
    }

    /// Stops all I/O and waits for the owned worker and descriptor to be released.
    ///
    /// # Errors
    /// Returns a closed-worker, worker-join or one-second reply error.
    pub async fn release_after_commit(&self) -> io::Result<()> {
        self.owner.admission.close();
        self.request(
            Control::Release,
            Duration::from_secs(1),
            "timed out waiting for PTY actor release",
        )
        .await?;
        let task = self
            .owner
            .task
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(task) = task {
            task.await.map_err(io::Error::other)?;
        }
        Ok(())
    }

    /// Sends a capacity-independent control and awaits its supplied reply budget.
    async fn request<T>(
        &self,
        command: fn(Reply<T>) -> Control,
        budget: Duration,
        text: &'static str,
    ) -> io::Result<T> {
        let (reply, receiver) = tokio::sync::oneshot::channel();
        self.owner
            .control
            .send(command(reply))
            .map_err(|_| closed())?;
        tokio::time::timeout(budget, receiver)
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, text))?
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, text))?
    }
}
