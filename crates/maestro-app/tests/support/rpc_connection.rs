//! A protocol connection over memory: the client writes lines and reads every record the
//! server writes, with recorded signal delivery and replaceable workspace operations.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::export_fixtures::{Recording, environment, published};
use maestro_app::rpc_mode::{
    RpcFuture, RpcInput, RpcLineReader, RpcProcessPort, WorkspaceEventSink, WorkspaceOperations,
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
use serde_json::{Value, json};
use std::{cell::RefCell, future::Future, io, pin::Pin, rc::Rc};
use tokio::sync::mpsc;

/// Lines a test supplies, read in order; a dropped sender ends the input.
struct MemoryReader {
    /// The supplied inputs.
    inputs: mpsc::UnboundedReceiver<RpcInput>,
}

impl RpcLineReader for MemoryReader {
    fn next_line(&mut self) -> Pin<Box<dyn Future<Output = io::Result<RpcInput>> + '_>> {
        Box::pin(async move {
            Ok(self
                .inputs
                .recv()
                .await
                .unwrap_or(RpcInput::End { tail: None }))
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
struct ClosureWorkspace(WorkspaceAnswer);

impl WorkspaceOperations for ClosureWorkspace {
    fn execute(
        &self,
        command: workspace::RpcCommand,
        events: WorkspaceEventSink,
    ) -> RpcFuture<WorkspaceResult> {
        Box::pin(std::future::ready((self.0)(command, events)))
    }
}

/// A test's end of a connection.
pub struct Client {
    /// Supplies the server's input.
    inputs: Option<mpsc::UnboundedSender<RpcInput>>,
    /// Every line the server wrote, in order.
    lines: mpsc::UnboundedReceiver<String>,
    /// The running server.
    pub server: tokio::task::JoinHandle<io::Result<RpcConnectionOutcome>>,
}

impl Client {
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
        self.inputs.as_ref().unwrap().send(input).unwrap();
    }

    /// The next line the server writes, as written.
    pub async fn next_line(&mut self) -> String {
        self.lines.recv().await.unwrap()
    }

    /// The next record the server writes.
    pub async fn next(&mut self) -> Value {
        serde_json::from_str(&self.next_line().await).unwrap()
    }

    /// The connection's outcome, once it ends.
    pub async fn outcome(self) -> io::Result<RpcConnectionOutcome> {
        self.server.await.unwrap()
    }
}

/// Serves `runtime` over memory with `workspace` answering workspace commands.
pub fn connect_with(runtime: Rc<AgentSessionRuntime>, workspace: WorkspaceAnswer) -> Client {
    let (inputs, received) = mpsc::unbounded_channel();
    let (written, lines) = mpsc::unbounded_channel::<String>();
    let signals = Rc::new(Signals::default());
    let options = RpcModeOptions {
        workspace: Rc::new(ClosureWorkspace(workspace)),
        process: signals,
        theme: published(),
        export_operations: Rc::new(Recording::default()),
        render_environment: environment(published()),
    };
    let output = Rc::new(move |line: &str| {
        written.send(line.to_owned()).unwrap();
        Ok(())
    });
    let reader = Box::new(MemoryReader { inputs: received });
    let server = tokio::task::spawn_local(run_rpc_connection(runtime, reader, output, options));
    Client {
        inputs: Some(inputs),
        lines,
        server,
    }
}

/// Serves `runtime` over memory; a workspace command fails as unsupported.
pub fn connect(runtime: Rc<AgentSessionRuntime>) -> Client {
    connect_with(
        runtime,
        Rc::new(|_, _| {
            Err(workspace_schema::ErrorBody {
                code: "unsupported".into(),
                message: "no workspace".into(),
            })
        }),
    )
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

impl Client {
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
