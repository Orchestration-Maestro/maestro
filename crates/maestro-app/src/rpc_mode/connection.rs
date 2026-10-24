//! One connection: line reading, the record each line holds, and the loop that admits lines
//! until the input ends or the connection shuts down.
use super::commands::Reply;
use super::lifecycle::Finish;
use super::output::write_record;
use super::ui::RpcUi;
use super::{RpcConnectionOutcome, RpcModeOptions, RpcOutput, RpcRecord};
use crate::agent_session::AgentSession;
use crate::rpc_types::{RpcCommandBody, RpcCommandRejection, RpcResponse, parse_rpc_command};
use crate::runtime::agent_session_runtime::AgentSessionRuntime;
use crate::utils::task::start_now;
use maestro_json::jsonl::JsonlLineSplitter;
use serde_json::Value;
use std::{cell::RefCell, collections::VecDeque, future::Future, io, pin::Pin, rc::Rc};
use tokio::io::{AsyncRead, AsyncReadExt};

/// What a line reader observed next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RpcInput {
    /// A complete line, without its LF and one preceding CR.
    Line(String),
    /// The input ended, after an unterminated final line when there was one.
    End {
        /// The unterminated final line.
        tail: Option<String>,
    },
}

/// Reads protocol lines from a caller-owned input.
pub trait RpcLineReader {
    /// The next line or the end of the input. Dropping the future before it settles loses
    /// no input.
    fn next_line(&mut self) -> Pin<Box<dyn Future<Output = io::Result<RpcInput>> + '_>>;
    /// Stops reading: every later call observes the end without a final line.
    fn detach(&mut self);
}

/// Reads LF-terminated lines from a byte stream through the shared splitter.
pub struct JsonlLineReader<R> {
    /// The byte stream.
    reader: R,
    /// The unfinished line.
    splitter: JsonlLineSplitter,
    /// Complete lines not yet returned.
    pending: VecDeque<String>,
    /// Whether the end was observed or reading stopped.
    ended: bool,
}

impl<R> JsonlLineReader<R> {
    /// A reader of `reader`.
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            splitter: JsonlLineSplitter::default(),
            pending: VecDeque::new(),
            ended: false,
        }
    }
}

impl<R: AsyncRead + Unpin> JsonlLineReader<R> {
    /// Reads until a line completes or the input ends.
    async fn read_line(&mut self) -> io::Result<RpcInput> {
        loop {
            if let Some(line) = self.pending.pop_front() {
                return Ok(RpcInput::Line(line));
            }
            if self.ended {
                return Ok(RpcInput::End { tail: None });
            }
            let mut bytes = Vec::with_capacity(8192);
            if self.reader.read_buf(&mut bytes).await? == 0 {
                self.ended = true;
                return Ok(RpcInput::End {
                    tail: self.splitter.finish(),
                });
            }
            self.pending.extend(self.splitter.push(&bytes));
        }
    }
}

impl<R: AsyncRead + Unpin> RpcLineReader for JsonlLineReader<R> {
    fn next_line(&mut self) -> Pin<Box<dyn Future<Output = io::Result<RpcInput>> + '_>> {
        Box::pin(self.read_line())
    }

    fn detach(&mut self) {
        self.ended = true;
        self.pending.clear();
    }
}

/// A line's record: a decoded JSON value, or the decoder's failure.
pub(super) type Record = Result<Value, serde_json::Error>;

/// The state one connection shares between its loop, its tasks and its callbacks.
pub(super) struct Connection {
    /// The session runtime.
    pub(super) runtime: Rc<AgentSessionRuntime>,
    /// The output.
    pub(super) output: RpcOutput,
    /// The extension interface.
    pub(super) ui: Rc<RpcUi>,
    /// The caller's facilities.
    pub(super) options: RpcModeOptions,
    /// Records admitted during the initial binding, in arrival order; `None` once bound.
    pub(super) early: RefCell<Option<Vec<Record>>>,
    /// Notifies every waiter once the initial binding has handled the records that arrived
    /// meanwhile.
    pub(super) bound: tokio::sync::Notify,
    /// Shutdown and subscription state.
    pub(super) lifecycle: RefCell<super::lifecycle::Lifecycle>,
}

/// Serves `runtime_host` over `input` and `output` until the input ends, an extension
/// requests shutdown or a termination signal arrives; returns how it ended.
///
/// The session is bound to the protocol's extension interface first; meanwhile dialog
/// responses are answered and every other line waits, in arrival order, until binding ends.
/// Commands run concurrently and answer as they finish. Must run inside a
/// [`tokio::task::LocalSet`].
///
/// # Errors
/// Returns an input or output failure, or the failure of binding or disposing the session.
pub async fn run_rpc_connection(
    runtime_host: Rc<AgentSessionRuntime>,
    mut input: Box<dyn RpcLineReader>,
    output: RpcOutput,
    options: RpcModeOptions,
) -> io::Result<RpcConnectionOutcome> {
    let (connection, mut finished) = Connection::start(runtime_host, output, options);
    let outcome = connection.serve(&mut *input, &mut finished).await;
    input.detach();
    outcome
}

impl Connection {
    /// Reads and admits lines until the connection finishes.
    async fn serve(
        self: &Rc<Self>,
        input: &mut dyn RpcLineReader,
        finished: &mut Finish,
    ) -> io::Result<RpcConnectionOutcome> {
        loop {
            let next = tokio::select! {
                biased;
                outcome = &mut *finished => return outcome.map_err(io::Error::other)?,
                next = input.next_line() => next,
            };
            match next {
                Ok(RpcInput::Line(line)) => self.accept(&line),
                Ok(RpcInput::End { tail }) => return self.end(tail, finished).await,
                Err(error) => {
                    self.fail(error);
                    return finished.await.map_err(io::Error::other)?;
                }
            }
        }
    }

    /// Ends the input: answers a final dialog response at once, then, once the initial
    /// binding is done, shuts down while handling any other final line, or finishes at once
    /// when a shutdown is already running.
    async fn end(
        self: &Rc<Self>,
        tail: Option<String>,
        finished: &mut Finish,
    ) -> io::Result<RpcConnectionOutcome> {
        if self.lifecycle.borrow().shutting_down {
            return Ok(RpcConnectionOutcome::Eof);
        }
        // A startup dialog may wait for this answer, so it comes before the binding's end.
        let tail = tail.and_then(|line| self.decode(&line));
        // Input is served after the initial binding, so its end waits for the binding too.
        tokio::select! {
            biased;
            outcome = &mut *finished => return outcome.map_err(io::Error::other)?,
            () = self.initial_binding() => {}
        }
        let shutdown =
            futures_util::future::join(self.shutdown(RpcConnectionOutcome::Eof), async {
                if let Some(record) = tail {
                    self.handle(record).await;
                }
            });
        let early = tokio::select! {
            biased;
            outcome = &mut *finished => Some(outcome),
            _ = shutdown => None,
        };
        match early {
            Some(outcome) => outcome.map_err(io::Error::other)?,
            None => finished.await.map_err(io::Error::other)?,
        }
    }

    /// Admits `line` and handles its record unless it was answered or held.
    fn accept(self: &Rc<Self>, line: &str) {
        if let Some(record) = self.admit(line) {
            self.spawn(record);
        }
    }

    /// Decodes `line`, answers a dialog response at once and holds every other record
    /// while the initial binding runs; returns the record to handle now.
    fn admit(&self, line: &str) -> Option<Record> {
        let record = self.decode(line)?;
        match &mut *self.early.borrow_mut() {
            Some(early) => {
                early.push(record);
                None
            }
            None => Some(record),
        }
    }

    /// Decodes `line` and answers a dialog response; returns any other record.
    fn decode(&self, line: &str) -> Option<Record> {
        let record = serde_json::from_str::<Value>(line);
        if let Ok(value) = &record
            && value.get("type").and_then(Value::as_str) == Some("extension_ui_response")
        {
            self.ui.answer(value);
            return None;
        }
        Some(record)
    }

    /// Handles `record` at once and finishes it on its own task, so commands overlap and a
    /// command that answers without waiting is answered before the next line is read.
    pub(super) fn spawn(self: &Rc<Self>, record: Record) {
        let connection = Rc::clone(self);
        let task = Box::pin(async move {
            connection.handle(record).await;
        });
        start_now(task, |task| drop(tokio::task::spawn_local(task)));
    }

    /// Answers `record`; a handled command checks for a requested shutdown afterwards.
    async fn handle(self: &Rc<Self>, record: Record) {
        match self.reply(record).await {
            Reply::Handled(response) => {
                if let Some(response) = response {
                    self.write(RpcRecord::Response(&response));
                }
                self.check_shutdown_requested().await;
            }
            Reply::Failed(response) => self.write(RpcRecord::Response(&response)),
        }
    }

    /// The reply to `record`: a command's answer, or the failure of a record that is not
    /// one.
    async fn reply(self: &Rc<Self>, record: Record) -> Reply {
        let value = match record {
            Ok(value) => value,
            Err(error) => {
                return Reply::Failed(RpcResponse::Error {
                    id: None,
                    command: Some("parse".into()),
                    error: format!("Failed to parse command: {error}"),
                });
            }
        };
        let command = match parse_rpc_command(&value) {
            Ok(command) => command,
            Err(RpcCommandRejection::Unknown(kind)) => {
                return Reply::Handled(Some(RpcResponse::Error {
                    id: None,
                    error: format!("Unknown command: {kind}"),
                    command: Some(kind),
                }));
            }
            Err(rejection) => return Reply::Failed(rejected(rejection)),
        };
        let kind = value["type"].as_str().unwrap_or_default().to_owned();
        match command.command {
            RpcCommandBody::Session(session) => {
                self.run_session_command(command.id, kind, session).await
            }
            RpcCommandBody::Workspace(workspace) => Reply::Handled(Some(
                self.run_workspace_command(command.id, kind, *workspace)
                    .await,
            )),
        }
    }

    /// The current session.
    pub(super) fn session(&self) -> Rc<AgentSession> {
        self.runtime.session()
    }

    /// Writes `record`; a failed write ends the connection with that failure.
    pub(super) fn write(self: &Rc<Self>, record: RpcRecord<'_>) {
        if let Err(error) = write_record(&self.output, record) {
            self.fail(error);
        }
    }
}

/// The failure response of a record that is not a valid command.
fn rejected(rejection: RpcCommandRejection) -> RpcResponse {
    match rejection {
        RpcCommandRejection::Invalid { command, id, error } => RpcResponse::Error {
            id,
            command: Some(command),
            error: error.to_string(),
        },
        other => RpcResponse::Error {
            id: None,
            command: None,
            error: other.to_string(),
        },
    }
}
