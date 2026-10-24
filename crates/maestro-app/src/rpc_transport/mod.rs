//! A typed client of a Maestro child running in RPC mode.
//!
//! [`RpcClient`] launches the child, owns its standard streams, forwards and keeps its
//! diagnostic text, and hands every record the child writes to its event callbacks. It
//! runs on the caller's Tokio local executor: it must be used inside a
//! [`tokio::task::LocalSet`], which drives the child's readers and the deadlines.
mod commands;
mod events;
mod process;
mod requests;
#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests;
mod types;

pub use types::{ModelInfo, RpcClientError, RpcClientOptions, RpcEventListener};

use events::ClientListener;
use maestro_listeners::Listeners;
use maestro_watch::fs_watch::NativeTimer;
use requests::Pending;
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
    time::Duration,
};
use tokio::{
    sync::{mpsc, oneshot, watch},
    time::Instant,
};

/// How long [`RpcClient::start`] lets the child run before checking that it is still alive.
const STARTUP_OBSERVATION: Duration = Duration::from_millis(100);

/// Controls one child at a time; it can be stopped and started again.
pub struct RpcClient {
    /// How the child is launched.
    options: RpcClientOptions,
    /// The state the client shares with its child's readers.
    inner: Rc<Inner>,
}

/// The state shared by a client and its child's readers.
#[derive(Default)]
struct Inner {
    /// The running child and what outlives it.
    state: RefCell<ClientState>,
    /// The event callbacks, kept across restarts.
    listeners: Rc<RefCell<Listeners<ClientListener>>>,
}

/// What a client keeps across its children.
#[derive(Default)]
struct ClientState {
    /// The current child, from start until stop.
    running: Option<Running>,
    /// The requests awaiting their answer, by identifier.
    pending: HashMap<String, Pending>,
    /// The number of the last request sent.
    request_id: u64,
    /// Every child's diagnostic text, in arrival order.
    stderr: String,
}

/// The handles of a started child.
struct Running {
    /// Queues a request's line, with its identifier, for the child's input; dropping it
    /// closes the pipe once the queued lines are written.
    input: mpsc::UnboundedSender<(String, String)>,
    /// Asks the child's driver to stop it and to report the termination; taken by the
    /// first stop.
    stop: Option<oneshot::Sender<oneshot::Sender<std::io::Result<()>>>>,
    /// Closes when the driver has finished with the child.
    done: watch::Receiver<()>,
    /// Queues the child's diagnostic bytes for this process's stderr.
    forward: std::sync::mpsc::Sender<process::Forward>,
    /// The exit status once the child has exited.
    exit: Option<std::process::ExitStatus>,
}

/// A deadline: it runs `expire` on `target` at `deadline`, on the local executor, while
/// `target` exists; dropping it first prevents that.
fn expiry_at<T: 'static>(
    deadline: Instant,
    target: Weak<T>,
    expire: impl FnOnce(&T) + 'static,
) -> NativeTimer {
    NativeTimer::due(
        deadline,
        Box::new(move || {
            if let Some(target) = target.upgrade() {
                expire(&target);
            }
        }),
    )
}

impl RpcClient {
    /// A client that launches its child with `options`; nothing starts yet.
    #[must_use]
    pub fn new(options: RpcClientOptions) -> Self {
        Self {
            options,
            inner: Rc::default(),
        }
    }

    /// Launches the child in RPC mode, starts reading its output and diagnostics, then
    /// reports a child that exits within its first 100 ms.
    ///
    /// # Errors
    /// Returns [`RpcClientError::AlreadyStarted`] while a child runs, the native launch
    /// failure, or [`RpcClientError::ProcessExit`] for a child that exited at once.
    pub async fn start(&self) -> Result<(), RpcClientError> {
        if self.inner.state.borrow().running.is_some() {
            return Err(RpcClientError::AlreadyStarted);
        }
        let mut command = process::command_for(&self.options)?;
        let done = process::launch(&self.inner, &mut command, process::forward_stderr).await?;
        tokio::time::sleep(STARTUP_OBSERVATION).await;
        let state = self.inner.state.borrow();
        let exit = state
            .running
            .as_ref()
            .filter(|running| running.done.same_channel(&done))
            .and_then(|running| running.exit);
        match exit {
            Some(status) => Err(RpcClientError::ProcessExit {
                code: status.code(),
                stderr: state.stderr.clone(),
            }),
            None => Ok(()),
        }
    }

    /// Stops reading the child's output, asks it to terminate, kills it if it is still
    /// running after one second, and waits until it is reaped and until every diagnostic
    /// byte read so far has been written to this process's stderr; it never waits for the
    /// end of that stream, which a descendant can hold open. Does nothing without a child.
    ///
    /// # Errors
    /// Returns the native failure of the forced kill.
    pub async fn stop(&self) -> Result<(), RpcClientError> {
        let (stop, mut done, forward) = {
            let mut state = self.inner.state.borrow_mut();
            let Some(running) = state.running.as_mut() else {
                return Ok(());
            };
            (
                running.stop.take(),
                running.done.clone(),
                running.forward.clone(),
            )
        };
        self.inner.fail_owned();
        let mut terminated = Ok(());
        if let Some(stop) = stop {
            let (report, reported) = oneshot::channel();
            if stop.send(report).is_ok() {
                terminated = reported.await.unwrap_or(Ok(()));
            }
        }
        let _ = done.changed().await;
        process::flush(&forward).await;
        let finished = {
            let mut state = self.inner.state.borrow_mut();
            let current = state
                .running
                .as_ref()
                .is_some_and(|running| running.done.same_channel(&done));
            current.then(|| state.running.take())
        };
        drop(finished);
        terminated.map_err(RpcClientError::Io)
    }

    /// The diagnostic text every child of this client has written so far.
    #[must_use]
    pub fn get_stderr(&self) -> String {
        self.inner.state.borrow().stderr.clone()
    }
}
