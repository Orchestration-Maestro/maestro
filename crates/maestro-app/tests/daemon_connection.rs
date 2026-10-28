//! Workspace-only connections over byte streams: acknowledged response writes, overlapping
//! commands, the input's end and the runner's cancellation.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{on_paused_task_set, until_idle};
use maestro_app::rpc_mode::{
    JsonlLineReader, RpcFuture, RpcLine, RpcModeOptions, RpcOutput, RpcRecord, WorkspaceEventSink,
    WorkspaceOperations,
};
use maestro_app::rpc_types::{workspace, workspace_schema};
use maestro_app::{RpcConnectionOutcome, RpcResponse, run_rpc_connection};
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    future::Future,
    io,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
    time::Duration,
};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, DuplexStream, ReadBuf};
use tokio::sync::{oneshot, watch};
use tokio::time::Instant;

/// A workspace command's result.
type WorkspaceResult = Result<workspace_schema::ResponseResult, workspace_schema::ErrorBody>;

/// A connection runner, polled by the test or a task.
type Runner = Pin<Box<dyn Future<Output = io::Result<RpcConnectionOutcome>>>>;

/// Workspace operations that keep each command's write completion: `ping` answers at once and
/// any other command waits until its operation is dropped, which they record.
struct Recorder {
    /// Each command's write completion, in dispatch order.
    completions: RefCell<Vec<oneshot::Receiver<()>>>,
    /// How many commands were dispatched.
    dispatched: watch::Sender<usize>,
    /// Whether a waiting operation was dropped.
    dropped: Rc<Cell<bool>>,
}

impl Recorder {
    /// Operations with nothing dispatched yet.
    fn new() -> Rc<Self> {
        Rc::new(Self {
            completions: RefCell::default(),
            dispatched: watch::Sender::new(0),
            dropped: Rc::default(),
        })
    }

    /// Waits until `count` commands were dispatched, then takes their write completions.
    async fn completions(&self, count: usize) -> Vec<oneshot::Receiver<()>> {
        self.dispatched
            .subscribe()
            .wait_for(|dispatched| *dispatched >= count)
            .await
            .unwrap();
        self.completions.take()
    }
}

/// Records that a waiting operation was dropped.
struct DropMark(Rc<Cell<bool>>);

impl Drop for DropMark {
    fn drop(&mut self) {
        self.0.set(true);
    }
}

impl WorkspaceOperations for Recorder {
    fn execute(
        &self,
        command: workspace::RpcCommand,
        _events: WorkspaceEventSink,
        write_complete: oneshot::Receiver<()>,
    ) -> RpcFuture<WorkspaceResult> {
        self.completions.borrow_mut().push(write_complete);
        self.dispatched.send_modify(|dispatched| *dispatched += 1);
        if matches!(command, workspace::RpcCommand::Ping(_)) {
            return Box::pin(std::future::ready(Ok(pong())));
        }
        let mark = DropMark(Rc::clone(&self.dropped));
        Box::pin(async move {
            let _mark = mark;
            std::future::pending().await
        })
    }
}

/// The answer to `ping`.
fn pong() -> workspace_schema::ResponseResult {
    workspace_schema::ResponseResult::Pong {
        version: "test".into(),
        protocol: 1,
        capabilities: None,
    }
}

/// The framed response to the `ping` with `id`.
fn pong_frame(id: &str) -> Vec<u8> {
    let response = RpcResponse::Workspace(Box::new(workspace::RpcResponse {
        id: Some(id.into()),
        command: "ping".into(),
        result: Ok(pong()),
    }));
    let line = RpcLine::new(&RpcRecord::Response(&response)).unwrap();
    line.as_str().as_bytes().to_vec()
}

/// A workspace-only connection of `recorder` writing to `output`: the test's input end and
/// the unpolled runner.
fn connect(
    recorder: &Rc<Recorder>,
    output: impl AsyncWrite + Unpin + 'static,
    stall_timeout: Option<Duration>,
) -> (DuplexStream, Runner) {
    let (input, server_input) = tokio::io::duplex(1024);
    let options = RpcModeOptions {
        workspace: Some(Rc::clone(recorder) as Rc<dyn WorkspaceOperations>),
        session: None,
    };
    let reader = Box::new(JsonlLineReader::new(server_input));
    let runner = run_rpc_connection(reader, RpcOutput::new(output, stall_timeout), options);
    (input, Box::pin(runner))
}

/// What a [`Valve`] lets through.
#[derive(Default)]
struct ValveState {
    /// How many more bytes a write may take.
    open: usize,
    /// Whether a flush completes.
    flushable: bool,
    /// Whether a write fails once no bytes are open.
    failing: bool,
    /// Every byte taken, in order.
    written: Vec<u8>,
    /// The writer waiting for bytes or a flush.
    waiting: Option<Waker>,
}

/// A writer taking only the bytes a test opens, flushing only once allowed and failing once
/// told to.
#[derive(Clone, Default)]
struct Valve(Rc<RefCell<ValveState>>);

impl Valve {
    /// Changes the state and wakes the waiting writer.
    fn change(&self, change: impl FnOnce(&mut ValveState)) {
        let mut state = self.0.borrow_mut();
        change(&mut state);
        if let Some(waker) = state.waiting.take() {
            waker.wake();
        }
    }

    /// Lets `bytes` more bytes through.
    fn open(&self, bytes: usize) {
        self.change(|state| state.open += bytes);
    }

    /// Lets flushes complete.
    fn allow_flush(&self) {
        self.change(|state| state.flushable = true);
    }

    /// Fails every write once the open bytes are taken.
    fn fail(&self) {
        self.change(|state| state.failing = true);
    }

    /// Every byte taken.
    fn written(&self) -> Vec<u8> {
        self.0.borrow().written.clone()
    }
}

impl AsyncWrite for Valve {
    fn poll_write(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        let mut state = self.0.borrow_mut();
        if state.open == 0 {
            if state.failing {
                return Poll::Ready(Err(io::Error::other("write failed")));
            }
            state.waiting = Some(context.waker().clone());
            return Poll::Pending;
        }
        let taken = state.open.min(bytes.len());
        state.open -= taken;
        state.written.extend_from_slice(&bytes[..taken]);
        Poll::Ready(Ok(taken))
    }

    fn poll_flush(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        let mut state = self.0.borrow_mut();
        if state.flushable {
            return Poll::Ready(Ok(()));
        }
        state.waiting = Some(context.waker().clone());
        Poll::Pending
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

/// Whether a write completion has settled: `Some(true)` once attempted, `Some(false)` once
/// dropped unattempted, `None` while pending.
fn settled(completion: &mut oneshot::Receiver<()>) -> Option<bool> {
    match completion.try_recv() {
        Ok(()) => Some(true),
        Err(oneshot::error::TryRecvError::Closed) => Some(false),
        Err(oneshot::error::TryRecvError::Empty) => None,
    }
}

/// Each response is acknowledged only once its own whole frame was written and flushed, in
/// the order the responses were admitted; a response dropped before any write attempt is
/// never acknowledged.
#[test]
fn physical_ack_and_order() {
    on_paused_task_set(|_| async {
        let recorder = Recorder::new();
        let valve = Valve::default();
        valve.allow_flush();
        let (mut input, runner) = connect(&recorder, valve.clone(), None);
        let server = tokio::task::spawn_local(runner);
        let lines = ["a", "b", "c"].map(|id| format!("{{\"id\":\"{id}\",\"type\":\"ping\"}}\n"));
        input.write_all(lines.concat().as_bytes()).await.unwrap();
        let mut completions = recorder.completions(3).await;
        assert_eq!(
            completions.iter_mut().map(settled).collect::<Vec<_>>(),
            [None; 3]
        );
        let (a, b) = (pong_frame("a"), pong_frame("b"));
        valve.open(a.len() - 1);
        until_idle().await;
        assert_eq!(valve.written(), a[..a.len() - 1]);
        assert_eq!(
            completions.iter_mut().map(settled).collect::<Vec<_>>(),
            [None; 3]
        );
        valve.open(1);
        until_idle().await;
        assert_eq!(valve.written(), a);
        let after_first: Vec<_> = completions.iter_mut().map(settled).collect();
        assert_eq!(after_first, [Some(true), None, None]);
        valve.open(5);
        valve.fail();
        let failure = server.await.unwrap().unwrap_err();
        assert_eq!(failure.to_string(), "write failed");
        assert_eq!(valve.written(), [a.as_slice(), &b[..5]].concat());
        let last: Vec<_> = completions[1..].iter_mut().map(settled).collect();
        assert_eq!(last, [Some(true), Some(false)]);
    });
}

/// A command waiting on the daemon does not hold up the next line: a later command split
/// across reads is answered whole, and the input's end drops the waiting operation before
/// the runner returns, with no reply invented for it.
#[test]
fn pending_duplex_and_eof() {
    on_paused_task_set(|_| async {
        let recorder = Recorder::new();
        let (client_output, output) = tokio::io::duplex(4096);
        let (mut input, runner) = connect(&recorder, output, None);
        let client_recorder = Rc::clone(&recorder);
        let client = tokio::task::spawn_local(async move {
            let mut output = tokio::io::BufReader::new(client_output);
            input
                .write_all(b"{\"id\":\"held\",\"type\":\"workspace.list\"}\n")
                .await
                .unwrap();
            client_recorder.completions(1).await;
            input.write_all(b"{\"id\":\"next\",\"ty").await.unwrap();
            until_idle().await;
            input.write_all(b"pe\":\"ping\"}\n").await.unwrap();
            let mut reply = String::new();
            output.read_line(&mut reply).await.unwrap();
            drop(input);
            let mut rest = String::new();
            tokio::io::AsyncReadExt::read_to_string(&mut output, &mut rest)
                .await
                .unwrap();
            (reply, rest)
        });
        let outcome = runner.await.unwrap();
        assert!(recorder.dropped.get(), "the waiting operation was dropped");
        assert_eq!(outcome, RpcConnectionOutcome::Eof);
        let (reply, rest) = client.await.unwrap();
        let reply: Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(reply["id"], "next");
        assert_eq!(reply["command"], "ping");
        assert_eq!(reply["success"], true);
        assert_eq!(rest, "");
    });
}

/// Dropping the runner while a command waits on the daemon drops that command's operation
/// and releases the connection's output instead of leaving them behind.
#[test]
fn runner_cancel_releases_pending_operation() {
    on_paused_task_set(|_| async {
        let recorder = Recorder::new();
        let (mut client_output, output) = tokio::io::duplex(4096);
        let (mut input, mut runner) = connect(&recorder, output, None);
        input
            .write_all(b"{\"id\":\"held\",\"type\":\"workspace.list\"}\n")
            .await
            .unwrap();
        let mut context = Context::from_waker(Waker::noop());
        assert!(runner.as_mut().poll(&mut context).is_pending());
        assert_eq!(*recorder.dispatched.borrow(), 1);
        drop(runner);
        until_idle().await;
        assert!(recorder.dropped.get(), "the waiting operation was dropped");
        let mut bytes = [0; 16];
        let mut read = ReadBuf::new(&mut bytes);
        let polled = Pin::new(&mut client_output).poll_read(&mut context, &mut read);
        assert!(
            matches!(polled, Poll::Ready(Ok(()))),
            "the output was released"
        );
        assert!(read.filled().is_empty());
    });
}

/// Dropping the runner during EOF draining releases the blocked writer and drops its
/// unacknowledged response rather than detaching the writer task.
#[test]
fn runner_cancel_during_eof_drain_releases_output() {
    on_paused_task_set(|_| async {
        let recorder = Recorder::new();
        let valve = Valve::default();
        let output = Rc::downgrade(&valve.0);
        let (mut input, mut runner) = connect(&recorder, valve, None);
        input
            .write_all(b"{\"id\":\"a\",\"type\":\"ping\"}\n")
            .await
            .unwrap();
        let mut context = Context::from_waker(Waker::noop());
        assert!(runner.as_mut().poll(&mut context).is_pending());
        let mut completions = recorder.completions(1).await;
        drop(input);
        assert!(runner.as_mut().poll(&mut context).is_pending());
        until_idle().await;
        assert!(output.upgrade().unwrap().borrow().waiting.is_some());
        assert_eq!(settled(&mut completions[0]), None);
        drop(runner);
        until_idle().await;
        assert!(output.upgrade().is_none(), "the output was released");
        assert_eq!(settled(&mut completions[0]), Some(false));
    });
}

/// A response whose write makes no progress is not acknowledged when admitted; at the stall
/// timeout its write fails, it is acknowledged as attempted and the connection ends with the
/// write's failure.
#[test]
fn stall_is_not_an_enqueue_ack() {
    on_paused_task_set(|_| async {
        let recorder = Recorder::new();
        let (mut input, runner) =
            connect(&recorder, Valve::default(), Some(Duration::from_secs(5)));
        let server = tokio::task::spawn_local(runner);
        input
            .write_all(b"{\"id\":\"a\",\"type\":\"ping\"}\n")
            .await
            .unwrap();
        let mut completions = recorder.completions(1).await;
        let start = Instant::now();
        assert_eq!(settled(&mut completions[0]), None);
        completions.pop().unwrap().await.unwrap();
        assert_eq!(start.elapsed(), Duration::from_secs(5));
        let failure = server.await.unwrap().unwrap_err();
        assert_eq!(failure.kind(), io::ErrorKind::TimedOut);
    });
}

/// A response that keeps making progress may take longer than the stall timeout in total:
/// each write that takes bytes restarts the budget, and the flush has its own.
#[test]
fn slow_progress_is_not_a_total_deadline() {
    on_paused_task_set(|_| async {
        let recorder = Recorder::new();
        let valve = Valve::default();
        let stall = Some(Duration::from_secs(5));
        let (mut input, runner) = connect(&recorder, valve.clone(), stall);
        let server = tokio::task::spawn_local(runner);
        input
            .write_all(b"{\"id\":\"a\",\"type\":\"ping\"}\n")
            .await
            .unwrap();
        let mut completions = recorder.completions(1).await;
        let start = Instant::now();
        let frame = pong_frame("a");
        let four_seconds = Duration::from_secs(4);
        for opened in [10, frame.len() - 10] {
            tokio::time::sleep(four_seconds).await;
            assert_eq!(settled(&mut completions[0]), None);
            valve.open(opened);
        }
        tokio::time::sleep(four_seconds).await;
        assert_eq!(settled(&mut completions[0]), None, "the flush is pending");
        valve.allow_flush();
        completions.pop().unwrap().await.unwrap();
        assert_eq!(start.elapsed(), Duration::from_secs(12));
        assert_eq!(valve.written(), frame);
        drop(input);
        assert_eq!(server.await.unwrap().unwrap(), RpcConnectionOutcome::Eof);
    });
}

/// The input's end drops waiting operations but still writes the responses already
/// admitted; a write failing after the connection ended is its result, and no later frame
/// follows the partial one.
#[test]
fn eof_drain_keeps_late_failure() {
    on_paused_task_set(|_| async {
        let recorder = Recorder::new();
        let valve = Valve::default();
        let (mut input, runner) = connect(&recorder, valve.clone(), None);
        let server = tokio::task::spawn_local(runner);
        let lines = concat!(
            "{\"id\":\"a\",\"type\":\"ping\"}\n",
            "{\"id\":\"b\",\"type\":\"ping\"}\n",
            "{\"id\":\"held\",\"type\":\"workspace.list\"}\n",
        );
        input.write_all(lines.as_bytes()).await.unwrap();
        let mut completions = recorder.completions(3).await;
        drop(input);
        until_idle().await;
        assert!(recorder.dropped.get(), "the waiting operation was dropped");
        assert!(
            !server.is_finished(),
            "admitted responses are still being written"
        );
        valve.open(5);
        valve.fail();
        let failure = server.await.unwrap().unwrap_err();
        assert_eq!(failure.to_string(), "write failed");
        assert_eq!(valve.written(), pong_frame("a")[..5]);
        let settled: Vec<_> = completions.iter_mut().map(settled).collect();
        assert_eq!(settled, [Some(true), Some(false), Some(false)]);
    });
}
