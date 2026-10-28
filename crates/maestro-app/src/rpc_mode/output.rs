//! The protocol's output: the records a server writes and the framed lines that carry them.
use crate::agent_session::AgentSessionEvent;
use crate::output_guard::write_raw_stdout;
use crate::rpc_types::{RpcExtensionUIRequest, RpcResponse, workspace_schema};
use maestro_extensions::ExtensionError;
use maestro_json::jsonl::serialize_json_line;
use serde::{Serialize, Serializer, ser::SerializeMap};
use std::{cell::RefCell, future::Future, io, rc::Rc, time::Duration};
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

/// One record a server writes.
///
/// Only these records are written; a JSON value is not one:
///
/// ```compile_fail
/// use maestro_app::rpc_mode::RpcLine;
/// let line = RpcLine::new(&serde_json::json!({"type": "response"}));
/// ```
#[derive(Clone, Copy)]
pub enum RpcRecord<'a> {
    /// A command's response.
    Response(&'a RpcResponse),
    /// A session event.
    Event(&'a AgentSessionEvent),
    /// A request of the extension interface.
    ExtensionUiRequest(&'a RpcExtensionUIRequest),
    /// An extension's failure, written without its stack.
    ExtensionError(&'a ExtensionError),
    /// A workspace event the connection opted into.
    WorkspaceEvent(&'a workspace_schema::EventData),
}

impl Serialize for RpcRecord<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Response(response) => response.serialize(serializer),
            Self::Event(event) => event.serialize(serializer),
            Self::ExtensionUiRequest(request) => request.serialize(serializer),
            Self::WorkspaceEvent(event) => event.serialize(serializer),
            Self::ExtensionError(error) => {
                let mut map = serializer.serialize_map(Some(4))?;
                map.serialize_entry("type", "extension_error")?;
                map.serialize_entry("extensionPath", &error.extension_path)?;
                map.serialize_entry("event", &error.event)?;
                map.serialize_entry("error", &error.error)?;
                map.end()
            }
        }
    }
}

/// One record as a compact JSON line, LF included.
///
/// A line holds a framed record and nothing else:
///
/// ```compile_fail
/// let line = maestro_app::rpc_mode::RpcLine(String::from("{}\n"));
/// ```
///
/// A framed line is written, never dropped:
///
/// ```compile_fail
/// #![deny(unused_must_use)]
/// use maestro_app::rpc_mode::{RpcLine, RpcRecord};
/// fn discard(record: &RpcRecord<'_>) -> serde_json::Result<()> {
///     RpcLine::new(record)?;
///     Ok(())
/// }
/// ```
#[must_use]
pub struct RpcLine(String);

impl RpcLine {
    /// Frames `record` through the shared JSONL framing.
    ///
    /// ```
    /// use maestro_app::rpc_mode::{RpcLine, RpcRecord};
    /// use maestro_app::RpcResponse;
    ///
    /// let response = RpcResponse::Error {
    ///     id: None,
    ///     command: None,
    ///     error: "failed".into(),
    /// };
    /// let line = RpcLine::new(&RpcRecord::Response(&response)).unwrap();
    /// assert!(line.as_str().ends_with('\n'));
    /// let _output =
    ///     maestro_app::rpc_mode::RpcOutput::synchronous(std::rc::Rc::new(|_: &RpcLine| Ok(())));
    /// ```
    ///
    /// # Errors
    /// Returns the serializer's failure.
    pub fn new(record: &RpcRecord<'_>) -> serde_json::Result<Self> {
        serialize_json_line(&serde_json::to_value(record)?).map(Self)
    }

    /// The line's text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Where a connection writes its lines.
///
/// Hand-built text never reaches it:
///
/// ```compile_fail
/// let output = maestro_app::rpc_mode::RpcOutput::synchronous(std::rc::Rc::new(
///     |line: &str| -> std::io::Result<()> { Ok(()) },
/// ));
/// ```
pub struct RpcOutput(pub(super) Transport);

/// Writes one line before returning.
type LineWriter = Rc<dyn Fn(&RpcLine) -> io::Result<()>>;

/// How an output writes its lines.
pub(super) enum Transport {
    /// Writes each line before returning.
    Synchronous(LineWriter),
    /// Writes each line in admission order on its own task, failing a write or flush that
    /// makes no progress within the stall timeout.
    Asynchronous(Box<dyn AsyncWrite + Unpin>, Option<Duration>),
}

impl RpcOutput {
    /// An output writing to `writer` on its own task, in admission order. A write or flush
    /// that makes no progress for `stall_timeout` fails the connection; each write that takes
    /// bytes starts a new budget.
    pub fn new(writer: impl AsyncWrite + Unpin + 'static, stall_timeout: Option<Duration>) -> Self {
        Self(Transport::Asynchronous(Box::new(writer), stall_timeout))
    }

    /// An output that writes each line through `writer` before the write returns.
    #[must_use]
    pub fn synchronous(writer: LineWriter) -> Self {
        Self(Transport::Synchronous(writer))
    }
}

/// An output writing each line to standard output, which carries protocol lines only once
/// taken over.
#[must_use]
pub fn stdout_output() -> RpcOutput {
    RpcOutput::synchronous(Rc::new(|line: &RpcLine| write_raw_stdout(line.as_str())))
}

/// Receives a line's write result once the write was attempted.
pub(super) type Receipt = Box<dyn FnOnce(&io::Result<()>)>;

/// What settles once one line's write was attempted.
pub(super) struct Attempt {
    /// Settles for the operation whose response the line carries, whatever the result.
    pub(super) complete: oneshot::Sender<()>,
    /// Receives the attempt's result.
    pub(super) receipt: Receipt,
}

/// One admitted line and what settles once its write was attempted.
struct Frame {
    /// The framed line.
    line: RpcLine,
    /// Settles once the whole line was written and flushed, or failed to be.
    attempted: Option<Attempt>,
}

/// A connection's running output.
pub(super) struct Writer(Running);

/// How a running output writes.
enum Running {
    /// Writes each line before returning.
    Synchronous(LineWriter),
    /// Admits each line for the writer task, which writes them in admission order.
    Queued {
        /// Admits lines; `None` once the output finished.
        frames: RefCell<Option<mpsc::UnboundedSender<Frame>>>,
        /// The writer task, until the output finished.
        task: RefCell<Option<JoinHandle<()>>>,
    },
}

impl Writer {
    /// Starts writing through `output`; a failed asynchronous write goes to `failed`.
    pub(super) fn start(output: RpcOutput, failed: impl FnOnce(io::Error) + 'static) -> Self {
        let (writer, stall_timeout) = match output.0 {
            Transport::Synchronous(write) => return Self(Running::Synchronous(write)),
            Transport::Asynchronous(writer, stall_timeout) => (writer, stall_timeout),
        };
        let (frames, admitted) = mpsc::unbounded_channel();
        let task = tokio::task::spawn_local(async move {
            if let Err(error) = drain(writer, stall_timeout, admitted).await {
                failed(error);
            }
        });
        Self(Running::Queued {
            frames: RefCell::new(Some(frames)),
            task: RefCell::new(Some(task)),
        })
    }

    /// Writes `line` now, or admits it for the writer task; `attempted` settles once the write
    /// was attempted, and disconnects if the line is dropped first.
    pub(super) fn write(&self, line: RpcLine, attempted: Option<Attempt>) -> io::Result<()> {
        match &self.0 {
            Running::Synchronous(write) => {
                let written = write(&line);
                settle(attempted, &written);
                written
            }
            Running::Queued { frames, .. } => frames
                .borrow()
                .as_ref()
                .and_then(|frames| frames.send(Frame { line, attempted }).ok())
                .ok_or_else(|| io::ErrorKind::BrokenPipe.into()),
        }
    }

    /// Stops admitting lines and waits until the admitted ones were written.
    pub(super) async fn finish(&self) {
        if let Running::Queued { frames, task } = &self.0 {
            frames.borrow_mut().take();
            let task = task.borrow_mut().take();
            if let Some(task) = task {
                drop(task.await);
            }
        }
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        if let Running::Queued { task, .. } = &mut self.0
            && let Some(task) = task.get_mut().take()
        {
            task.abort();
        }
    }
}

/// Writes each admitted frame in admission order until admission ends. The first failure
/// ends the writing and, by dropping `admitted`, admission: a later frame would follow a
/// partial one.
async fn drain(
    mut writer: Box<dyn AsyncWrite + Unpin>,
    stall_timeout: Option<Duration>,
    mut admitted: mpsc::UnboundedReceiver<Frame>,
) -> io::Result<()> {
    while let Some(frame) = admitted.recv().await {
        let line = frame.line.as_str().as_bytes();
        let written = write_line(&mut *writer, line, stall_timeout).await;
        settle(frame.attempted, &written);
        written?;
    }
    Ok(())
}

/// Reports that a line's write was attempted, and its result.
fn settle(attempted: Option<Attempt>, written: &io::Result<()>) {
    if let Some(attempted) = attempted {
        let _ = attempted.complete.send(());
        (attempted.receipt)(written);
    }
}

/// Writes all of `line`, then flushes it; a write or flush making no progress for
/// `stall_timeout` fails, and each write that takes bytes starts a new budget.
async fn write_line(
    writer: &mut (dyn AsyncWrite + Unpin),
    mut line: &[u8],
    stall_timeout: Option<Duration>,
) -> io::Result<()> {
    while !line.is_empty() {
        match within(stall_timeout, writer.write(line)).await? {
            0 => return Err(io::ErrorKind::WriteZero.into()),
            written => line = line.get(written..).unwrap_or_default(),
        }
    }
    within(stall_timeout, writer.flush()).await
}

/// Runs `operation`, failing it as timed out once `limit` passes.
async fn within<T>(
    limit: Option<Duration>,
    operation: impl Future<Output = io::Result<T>>,
) -> io::Result<T> {
    match limit {
        Some(limit) => tokio::time::timeout(limit, operation)
            .await
            .unwrap_or_else(|_| Err(io::ErrorKind::TimedOut.into())),
        None => operation.await,
    }
}
