//! A protocol connection over memory: the client writes lines and reads every record the
//! server writes, with recorded signal delivery and replaceable workspace operations.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::export_fixtures::{Recording, environment, published};
use crate::support::session_harness::answer;
use maestro_app::rpc_mode::{
    RpcFuture, RpcInput, RpcLine, RpcLineReader, RpcOutput, RpcProcessPort, WorkspaceEventSink,
    WorkspaceOperations,
};
use maestro_app::rpc_types::{workspace, workspace_schema};
use maestro_app::{
    AgentSessionRuntime, RpcConnectionOutcome, RpcModeOptions, TerminationSignal,
    run_rpc_connection,
};
use maestro_extensions::{
    ExtensionFactory, SessionBeforeForkEvent, SessionBeforeForkResult, SessionBeforeSwitchEvent,
    SessionBeforeSwitchResult, SessionStartEvent,
};
use maestro_models::Local;
use maestro_session::Presence;
use maestro_theme::LiveTheme;
use serde_json::{Value, json};
use std::{cell::RefCell, future::Future, io, pin::Pin, rc::Rc, task::Poll};
use tokio::sync::mpsc;

/// Polls `future` once without a waker.
pub fn poll_once<F: Future + ?Sized>(future: Pin<&mut F>) -> Poll<F::Output> {
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    future.poll(&mut context)
}

/// Lines a test supplies, read in order; a dropped sender ends the input.
struct MemoryReader {
    /// The supplied inputs and read failures.
    inputs: mpsc::UnboundedReceiver<io::Result<RpcInput>>,
}

impl RpcLineReader for MemoryReader {
    fn next_line(&mut self) -> Pin<Box<dyn Future<Output = io::Result<RpcInput>> + '_>> {
        Box::pin(async move {
            self.inputs
                .recv()
                .await
                .unwrap_or(Ok(RpcInput::End { tail: None }))
        })
    }

    fn detach(&mut self) {
        self.inputs.close();
    }
}

/// Delivers a termination signal to the server.
pub type SignalDelivery = Rc<dyn Fn(TerminationSignal)>;

/// Recorded signal registration: the delivery callback and how often delivery stopped.
#[derive(Default)]
pub struct Signals {
    /// The registered delivery.
    pub deliver: RefCell<Option<SignalDelivery>>,
    /// How often delivery was stopped.
    pub stopped: Rc<RefCell<usize>>,
}

impl RpcProcessPort for Signals {
    fn register_signals(
        &self,
        callback: Rc<dyn Fn(TerminationSignal)>,
    ) -> io::Result<Box<dyn Fn()>> {
        *self.deliver.borrow_mut() = Some(callback);
        let stopped = Rc::clone(&self.stopped);
        Ok(Box::new(move || *stopped.borrow_mut() += 1))
    }
}

/// A workspace command's result.
pub type WorkspaceResult = Result<workspace_schema::ResponseResult, workspace_schema::ErrorBody>;

/// Workspace operations answered by a test closure.
pub type WorkspaceAnswer = Rc<dyn Fn(workspace::RpcCommand, WorkspaceEventSink) -> WorkspaceResult>;

/// Workspace operations that answer through a test closure.
pub struct ClosureWorkspace(pub WorkspaceAnswer);

impl WorkspaceOperations for ClosureWorkspace {
    fn execute(
        &self,
        command: workspace::RpcCommand,
        events: WorkspaceEventSink,
        _write_complete: tokio::sync::oneshot::Receiver<()>,
    ) -> RpcFuture<WorkspaceResult> {
        Box::pin(std::future::ready((self.0)(command, events)))
    }
}

/// A test's end of a connection, whoever drives the server.
pub struct Peer {
    /// Supplies the server's input.
    inputs: Option<mpsc::UnboundedSender<io::Result<RpcInput>>>,
    /// Whether the server's writes fail.
    failing_writes: Rc<std::cell::Cell<bool>>,
    /// Every line the server wrote, in order.
    lines: mpsc::UnboundedReceiver<String>,
    /// The server's signal registration.
    signals: Rc<Signals>,
}

/// A test's end of a connection whose server runs on its own task.
pub struct Client {
    /// The test's end.
    peer: Peer,
    /// The running server.
    pub server: tokio::task::JoinHandle<io::Result<RpcConnectionOutcome>>,
}

impl std::ops::Deref for Client {
    type Target = Peer;

    fn deref(&self) -> &Peer {
        &self.peer
    }
}

impl std::ops::DerefMut for Client {
    fn deref_mut(&mut self) -> &mut Peer {
        &mut self.peer
    }
}

impl Client {
    /// The connection's outcome, once it ends.
    pub async fn outcome(self) -> io::Result<RpcConnectionOutcome> {
        self.server.await.unwrap()
    }
}

impl Peer {
    /// Writes `line` to the server.
    pub fn send(&self, line: &str) {
        self.input(RpcInput::Line(line.to_owned()));
    }

    /// Ends the input after the unterminated `tail`.
    pub fn end(&self, tail: Option<&str>) {
        self.input(RpcInput::End {
            tail: tail.map(str::to_owned),
        });
    }

    /// Supplies `input`.
    fn input(&self, input: RpcInput) {
        self.inputs.as_ref().unwrap().send(Ok(input)).unwrap();
    }

    /// Makes the server's next read fail.
    pub fn fail_read(&self) {
        let failure = io::Error::other("read failed");
        self.inputs.as_ref().unwrap().send(Err(failure)).unwrap();
    }

    /// Makes every later server write fail.
    pub fn fail_writes(&self) {
        self.failing_writes.set(true);
    }

    /// How often the server stopped signal delivery.
    pub fn signals_stopped(&self) -> usize {
        *self.signals.stopped.borrow()
    }

    /// The next line the server writes, as written.
    pub async fn next_line(&mut self) -> String {
        self.lines.recv().await.unwrap()
    }

    /// The next record the server writes.
    pub async fn next(&mut self) -> Value {
        serde_json::from_str(&self.next_line().await).unwrap()
    }

    /// Every line written and not yet read, as records.
    pub fn written(&mut self) -> Vec<Value> {
        std::iter::from_fn(|| self.lines.try_recv().ok())
            .map(|line| serde_json::from_str(&line).unwrap())
            .collect()
    }

    /// Delivers `signal` to the server.
    pub fn signal(&self, signal: TerminationSignal) {
        let deliver = self.signals.deliver.borrow().clone();
        deliver.unwrap()(signal);
    }
}

/// Serves `runtime` over memory with `workspace` answering workspace commands.
pub fn connect_with(runtime: Rc<AgentSessionRuntime>, workspace: WorkspaceAnswer) -> Client {
    serve(runtime, workspace, published())
}

/// Serves `runtime` over memory with `workspace` answering workspace commands and `theme`
/// as the live theme.
pub fn serve(
    runtime: Rc<AgentSessionRuntime>,
    workspace: WorkspaceAnswer,
    theme: LiveTheme,
) -> Client {
    let (peer, server) = unspawned(runtime, workspace, theme);
    let server = tokio::task::spawn_local(server);
    Client { peer, server }
}

/// The connection's server future, before anything polls it.
pub type Server = Pin<Box<dyn Future<Output = io::Result<RpcConnectionOutcome>>>>;

/// The test's end of a connection serving `runtime` over memory, and its server future,
/// which the test polls itself.
pub fn unspawned(
    runtime: Rc<AgentSessionRuntime>,
    workspace: WorkspaceAnswer,
    theme: LiveTheme,
) -> (Peer, Server) {
    let (inputs, received) = mpsc::unbounded_channel();
    let (written, lines) = mpsc::unbounded_channel::<String>();
    let signals = Rc::new(Signals::default());
    let options = RpcModeOptions {
        workspace: Some(Rc::new(ClosureWorkspace(workspace))),
        session: Some(maestro_app::RpcSessionOptions {
            runtime,
            process: Rc::clone(&signals) as Rc<dyn RpcProcessPort>,
            theme,
            export_operations: Rc::new(Recording::default()),
            render_environment: environment(published()),
        }),
    };
    let failing_writes = Rc::new(std::cell::Cell::new(false));
    let failing = Rc::clone(&failing_writes);
    let output = RpcOutput::synchronous(Rc::new(move |line: &RpcLine| {
        if failing.get() {
            return Err(io::Error::other("write failed"));
        }
        written.send(line.as_str().to_owned()).unwrap();
        Ok(())
    }));
    let reader = Box::new(MemoryReader { inputs: received });
    let peer = Peer {
        inputs: Some(inputs),
        failing_writes,
        lines,
        signals,
    };
    let server = Box::pin(run_rpc_connection(reader, output, options));
    (peer, server)
}

/// Serves `runtime` over memory; a workspace command fails as unsupported.
pub fn connect(runtime: Rc<AgentSessionRuntime>) -> Client {
    connect_with(runtime, no_workspace())
}

/// Workspace operations that fail every command as unsupported.
pub fn no_workspace() -> WorkspaceAnswer {
    Rc::new(|_, _| {
        Err(workspace_schema::ErrorBody {
            code: "unsupported".into(),
            message: "no workspace".into(),
        })
    })
}

/// An extension that records the reason of each `session_start` into `starts` and cancels
/// every switch and fork while `veto` is set.
pub fn replacement_watch(
    starts: Rc<RefCell<Vec<Value>>>,
    veto: Rc<std::cell::Cell<bool>>,
) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let (starts, switch_veto, fork_veto) =
            (Rc::clone(&starts), Rc::clone(&veto), Rc::clone(&veto));
        let started = api.on::<SessionStartEvent, ()>(
            "session_start".into(),
            Rc::new(move |event, _| {
                starts.borrow_mut().push(json!(event.reason));
                Box::pin(std::future::ready(Ok(None)))
            }),
        );
        let switched = api.on::<SessionBeforeSwitchEvent, SessionBeforeSwitchResult>(
            "session_before_switch".into(),
            Rc::new(move |_, _| {
                let cancel = Presence::Present(switch_veto.get());
                Box::pin(std::future::ready(Ok(Some(SessionBeforeSwitchResult {
                    cancel,
                }))))
            }),
        );
        let forked = api.on::<SessionBeforeForkEvent, SessionBeforeForkResult>(
            "session_before_fork".into(),
            Rc::new(move |_, _| {
                let cancel = Presence::Present(fork_veto.get());
                Box::pin(std::future::ready(Ok(Some(SessionBeforeForkResult {
                    cancel,
                    skip_conversation_restore: Presence::Missing,
                }))))
            }),
        );
        Box::pin(std::future::ready(started.and(switched).and(forked)))
    })
}

impl Peer {
    /// Reads records up to and including the first that `last` accepts.
    pub async fn until(&mut self, last: impl Fn(&Value) -> bool) -> Vec<Value> {
        let mut records = Vec::new();
        loop {
            let record = self.next().await;
            let done = last(&record);
            records.push(record);
            if done {
                return records;
            }
        }
    }

    /// Sends `line` and reads up to its response, returning the response.
    pub async fn ask(&mut self, line: &str) -> Value {
        self.send(line);
        self.until(|record| record["type"] == "response")
            .await
            .pop()
            .unwrap()
    }
}

/// Whether `record` is the response to the command `command`.
pub fn response_to(command: &'static str) -> impl Fn(&Value) -> bool {
    move |record| record["type"] == "response" && record["command"] == command
}

/// The records of `kind`.
pub fn of_type<'a>(records: &'a [Value], kind: &str) -> Vec<&'a Value> {
    records
        .iter()
        .filter(|record| record["type"] == kind)
        .collect()
}

/// Appends a user message with `text` and its answer to the current session, which writes
/// the session file; returns the user message's entry.
pub fn user_entry(runtime: &AgentSessionRuntime, text: &str) -> String {
    let session = runtime.session();
    let mut manager = session.session_manager().borrow_mut();
    let entry = manager
        .append_message(json!({"role": "user", "content": text, "timestamp": 1}))
        .unwrap();
    manager
        .append_message(serde_json::to_value(answer("answered", None, None)).unwrap())
        .unwrap();
    entry
}

/// The recorded outcome of each selection, or its failure text.
pub type Outcomes = Rc<RefCell<Vec<Result<Option<String>, String>>>>;

/// An extension whose `session_start` handler asks a selection and records its outcome.
pub fn startup_select(outcomes: Outcomes) -> ExtensionFactory<Local> {
    select_on::<SessionStartEvent>("session_start", "start", outcomes)
}

/// An extension whose handler of `event`, carrying `V`, asks the selection `title` and
/// records its outcome.
pub fn select_on<V: 'static>(
    event: &'static str,
    title: &'static str,
    outcomes: Outcomes,
) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let outcomes = Rc::clone(&outcomes);
        let registered = api.on::<V, ()>(
            event.into(),
            Rc::new(move |_, ctx| {
                let outcomes = Rc::clone(&outcomes);
                Box::pin(async move {
                    let select = ctx.ui()?.select(title.into(), vec!["a".into()], None);
                    let outcome = select.await.map_err(|error| error.to_string());
                    outcomes.borrow_mut().push(outcome);
                    Ok(None)
                })
            }),
        );
        Box::pin(std::future::ready(registered))
    })
}
