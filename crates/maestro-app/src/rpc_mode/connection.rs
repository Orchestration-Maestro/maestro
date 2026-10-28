//! One connection: line reading, the record each line holds, and the loop that admits lines
//! until the input ends or the connection shuts down.
use super::commands::Reply;
use super::lifecycle::{Finish, Lifecycle};
use super::output::{Attempt, Writer};
use super::ui::RpcUi;
use super::{
    RpcConnectionOutcome, RpcLine, RpcModeOptions, RpcOutput, RpcRecord, RpcSessionOptions,
    WorkspaceOperations,
};
use crate::agent_session::AgentSession;
use crate::rpc_types::{
    RpcCommandBody, RpcCommandRejection, RpcResponse, names, parse_rpc_command, workspace,
    workspace_schema,
};
use crate::utils::task::start_now;
use maestro_json::jsonl::JsonlLineSplitter;
use serde_json::Value;
use std::{
    cell::RefCell,
    collections::VecDeque,
    future::Future,
    io,
    pin::Pin,
    rc::{Rc, Weak},
};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::task::JoinSet;

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
    /// Complete lines, and a failure that ends them, not yet returned.
    pending: VecDeque<io::Result<String>>,
    /// The raw byte limit of a strictly decoded line; `None` decodes lossily.
    max_bytes: Option<usize>,
    /// Whether the end was observed or reading stopped.
    ended: bool,
}

impl<R> JsonlLineReader<R> {
    /// A reader of `reader`, decoding invalid UTF-8 with replacement characters.
    pub fn new(reader: R) -> Self {
        Self::reading(reader, None)
    }

    /// A reader of `reader` that decodes each line as strict UTF-8 and fails a line whose
    /// bytes before its LF exceed `max_bytes`, a CR included; it reads only LF-terminated
    /// lines, so an unterminated final line is dropped.
    pub fn strict(reader: R, max_bytes: usize) -> Self {
        Self::reading(reader, Some(max_bytes))
    }

    /// A reader of `reader` with the strict limit `max_bytes`, if any.
    fn reading(reader: R, max_bytes: Option<usize>) -> Self {
        Self {
            reader,
            splitter: JsonlLineSplitter::default(),
            pending: VecDeque::new(),
            max_bytes,
            ended: false,
        }
    }
}

impl<R: AsyncRead + Unpin> JsonlLineReader<R> {
    /// Reads until a line completes or the input ends.
    async fn read_line(&mut self) -> io::Result<RpcInput> {
        loop {
            if let Some(line) = self.pending.pop_front() {
                return line.map(RpcInput::Line);
            }
            if self.ended {
                return Ok(RpcInput::End { tail: None });
            }
            let mut bytes = Vec::with_capacity(8192);
            if self.reader.read_buf(&mut bytes).await? == 0 {
                self.ended = true;
                let tail = match self.max_bytes {
                    Some(_) => None,
                    None => self.splitter.finish(),
                };
                return Ok(RpcInput::End { tail });
            }
            match self.max_bytes {
                Some(max_bytes) => self
                    .pending
                    .extend(self.splitter.push_strict(&bytes, max_bytes)),
                None => self
                    .pending
                    .extend(self.splitter.push(&bytes).into_iter().map(Ok)),
            }
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
    /// The session facilities, absent on workspace-only connections.
    pub(super) session: Option<Rc<SessionConnection>>,
    /// The output, which only [`Connection::send`] reaches.
    writer: Writer,
    /// The commands still running.
    tasks: RefCell<JoinSet<()>>,
    /// The caller's workspace operations.
    pub(super) workspace: Option<Rc<dyn WorkspaceOperations>>,
    /// Records admitted during the initial binding, in arrival order; `None` once bound.
    pub(super) early: RefCell<Option<Vec<Record>>>,
    /// Notifies every waiter once the initial binding has handled the records that arrived
    /// meanwhile.
    pub(super) bound: tokio::sync::Notify,
    /// Shutdown and subscription state.
    pub(super) lifecycle: RefCell<Lifecycle>,
}

/// Session-only state, with a non-owning route to the shared connection writer.
pub(super) struct SessionConnection {
    /// The session's runtime and native facilities.
    pub(super) options: RpcSessionOptions,
    /// The extension interface shared across session rebindings.
    pub(super) ui: Rc<RpcUi>,
    /// The shared protocol writer's owner.
    connection: Weak<Connection>,
}

impl SessionConnection {
    /// The current session.
    pub(super) fn session(&self) -> Rc<AgentSession> {
        self.options.runtime.session()
    }

    /// Writes a session record through the connection's only writer.
    pub(super) fn write(&self, record: RpcRecord<'_>) -> io::Result<()> {
        write_to(&self.connection, record)
    }
}

/// Serves the supplied workspace and optional session over `input` and `output` until the input ends, an extension
/// requests shutdown or a termination signal arrives; returns how it ended.
///
/// A supplied session is bound to the protocol's extension interface first; meanwhile dialog
/// responses are answered and every other line waits, in arrival order, until binding ends.
/// Commands run concurrently and answer as they finish. Lines already admitted to `output`
/// are written before this returns; commands still running are dropped. Must run inside a
/// [`tokio::task::LocalSet`].
///
/// # Errors
/// Returns an input or output failure, or the failure of binding or disposing the session.
pub async fn run_rpc_connection(
    mut input: Box<dyn RpcLineReader>,
    output: RpcOutput,
    options: RpcModeOptions,
) -> io::Result<RpcConnectionOutcome> {
    let (connection, mut finished) = Connection::start(output, options);
    let _commands = Commands(Rc::clone(&connection));
    let outcome = connection.serve(&mut *input, &mut finished).await;
    input.detach();
    connection.writer.finish().await;
    let late = connection.lifecycle.borrow_mut().take_failure();
    outcome.and_then(|outcome| late.map_or(Ok(outcome), Err))
}

/// Drops the connection's running commands once the runner ends or is dropped.
struct Commands(Rc<Connection>);

impl Drop for Commands {
    fn drop(&mut self) {
        drop(self.0.tasks.take());
    }
}

impl Connection {
    /// A connection with its workspace operations and optional unbound session.
    pub(super) fn new(
        output: RpcOutput,
        options: RpcModeOptions,
        lifecycle: Lifecycle,
    ) -> Rc<Self> {
        Rc::new_cyclic(|connection| Self {
            session: options.session.map(|options| {
                Rc::new(SessionConnection {
                    ui: Rc::new(RpcUi::new(Weak::clone(connection), options.theme.clone())),
                    options,
                    connection: Weak::clone(connection),
                })
            }),
            writer: Writer::start(output, failing(Weak::clone(connection))),
            tasks: RefCell::default(),
            workspace: options.workspace,
            early: RefCell::new(Some(Vec::new())),
            bound: tokio::sync::Notify::new(),
            lifecycle: RefCell::new(lifecycle),
        })
    }

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
    /// binding is done, shuts down while handling any other final line, or repeats the
    /// shutdown that already owns the teardown.
    async fn end(
        self: &Rc<Self>,
        tail: Option<String>,
        finished: &mut Finish,
    ) -> io::Result<RpcConnectionOutcome> {
        if self.shutting_down() {
            self.shutdown(RpcConnectionOutcome::Eof).await;
            return finished.await.map_err(io::Error::other)?;
        }
        if self.session.is_none() {
            return self.end_without_session(tail, finished).await;
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

    /// Ends the input of a connection without a session: a final command starts, the
    /// commands still waiting are dropped, since nobody can read their answers, and the
    /// connection shuts down.
    async fn end_without_session(
        self: &Rc<Self>,
        tail: Option<String>,
        finished: &mut Finish,
    ) -> io::Result<RpcConnectionOutcome> {
        if let Some(record) = tail.and_then(|line| self.decode(&line)) {
            self.spawn(record);
        }
        self.tasks.take().shutdown().await;
        self.shutdown(RpcConnectionOutcome::Eof).await;
        finished.await.map_err(io::Error::other)?
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
            if let Some(session) = &self.session {
                session.ui.answer(value);
            }
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
        start_now(task, |task| {
            let mut tasks = self.tasks.borrow_mut();
            while tasks.try_join_next().is_some() {}
            tasks.spawn_local(task);
        });
    }

    /// Answers `record`; a handled command checks for a requested shutdown afterwards.
    async fn handle(self: &Rc<Self>, record: Record) {
        match self.reply(record).await {
            Reply::Handled(response) => {
                if let Some(response) = response {
                    drop(self.write(RpcRecord::Response(&response)));
                }
                self.check_shutdown_requested().await;
            }
            Reply::Failed(response) => drop(self.write(RpcRecord::Response(&response))),
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
            RpcCommandBody::Session(session) => match &self.session {
                Some(owner) => owner.run_session_command(command.id, kind, session).await,
                None => Reply::Handled(Some(RpcResponse::Error {
                    id: command.id,
                    command: Some(kind),
                    error: "Session command unavailable; open in a session to use it".into(),
                })),
            },
            RpcCommandBody::Workspace(workspace) => {
                self.run_workspace_command(command.id, kind, *workspace)
                    .await
            }
        }
    }

    /// Writes `record` immediately for synchronous output, or queues it for asynchronous output.
    /// Serialization, admission and synchronous write failures are returned and end the
    /// connection; asynchronous write failures end the connection later.
    pub(super) fn write(self: &Rc<Self>, record: RpcRecord<'_>) -> io::Result<()> {
        self.send(record, None)
    }

    /// Writes `record` as [`Connection::write`] does; `attempted` settles once the line's
    /// write was attempted, and disconnects if the line is dropped first.
    pub(super) fn send(
        self: &Rc<Self>,
        record: RpcRecord<'_>,
        attempted: Option<Attempt>,
    ) -> io::Result<()> {
        let written = RpcLine::new(&record)
            .map_err(io::Error::from)
            .and_then(|line| self.writer.write(line, attempted));
        written.map_err(|error| {
            let reported = io::Error::new(error.kind(), error.to_string());
            self.fail(error);
            reported
        })
    }
}

/// Ends `connection` with a failure its writer task reports, while the connection lasts.
fn failing(connection: Weak<Connection>) -> impl FnOnce(io::Error) {
    move |error| {
        if let Some(connection) = connection.upgrade() {
            connection.fail(error);
        }
    }
}

/// Writes `record` on `connection` while the connection lasts.
pub(super) fn write_to(connection: &Weak<Connection>, record: RpcRecord<'_>) -> io::Result<()> {
    connection
        .upgrade()
        .ok_or_else(|| io::Error::other("the connection has ended"))?
        .write(record)
}

/// The failure response of a record that is not a valid command; a known workspace type
/// answers with the workspace envelope, an empty id and `invalid_request`.
fn rejected(rejection: RpcCommandRejection) -> RpcResponse {
    match rejection {
        RpcCommandRejection::Invalid { command, error, .. }
            if names::<workspace::RpcCommand>(&command) =>
        {
            RpcResponse::Workspace(Box::new(workspace::RpcResponse {
                id: Some(String::new()),
                command,
                result: Err(workspace_schema::ErrorBody {
                    code: "invalid_request".into(),
                    message: format!("invalid request: {error}"),
                }),
            }))
        }
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
