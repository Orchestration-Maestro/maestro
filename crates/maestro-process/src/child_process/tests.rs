//! Private collector, grace and escalation behaviour.
#![cfg(unix)]
use super::{
    CapturedChild, SpawnSyncError, Termination, WaitOptions, capture, fill, wait_for_child_process,
};
use super::{OutputMode, SpawnSyncOptions};
use children::async_command;
use maestro_test_process::HARNESS_START;
use maestro_test_process::child::{collect_output, harness};
use std::io::Read;
use std::{
    cell::Cell,
    future::Future,
    io,
    os::unix::process::ExitStatusExt,
    path::PathBuf,
    pin::Pin,
    rc::Rc,
    sync::atomic::{AtomicU32, Ordering},
    task::{Context, Poll},
    time::Duration,
};
use tokio::io::{AsyncRead, AsyncReadExt, ReadBuf};
use tokio::net::TcpListener;
use tokio::process::Command;

pub(super) mod children;

/// A reader that hands the consumer exactly one chunk per read.
struct Chunks<'a>(std::vec::IntoIter<&'a [u8]>);
impl AsyncRead for Chunks<'_> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if let Some(chunk) = self.0.next() {
            buf.put_slice(chunk);
        }
        Poll::Ready(Ok(()))
    }
}

/// A distinctive error payload.
#[derive(Debug)]
struct Marker;
impl std::fmt::Display for Marker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("marker")
    }
}
impl std::error::Error for Marker {}

/// A reader whose first read fails with the marker.
struct Failing;
impl AsyncRead for Failing {
    fn poll_read(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        _: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Poll::Ready(Err(io::Error::other(Marker)))
    }
}

/// Reads `pipe` to its end into a fresh buffer.
async fn drain(mut pipe: Option<impl AsyncRead + Unpin>, keep: bool) -> io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    while pipe.is_some() {
        fill(&mut pipe, &mut buf, keep).await?;
    }
    Ok(buf)
}

use crate::tests::poll::is_pending;
use crate::tests::runtime::{paused_runtime, runtime};

#[test]
fn native_kill_rejects_nonpositive_and_unrepresentable_process_ids() {
    for pid in [0, u32::MAX] {
        for error in [super::kill_process(pid), super::kill_process_group(pid)] {
            assert_eq!(error.unwrap_err().kind(), io::ErrorKind::InvalidInput);
        }
    }
}

#[test]
fn windows_tree_kill_keeps_taskkill_operands() {
    let command = super::native::tree_kill_command(4_294_967_295);
    assert_eq!(command.get_program(), "taskkill");
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        ["/F", "/T", "/PID", "4294967295"]
    );
}

#[test]
fn bytes_split_across_consumed_reads_decode_as_one_character() {
    let scalar = "\u{10348}".as_bytes();
    for split in 1..scalar.len() {
        let (head, tail) = scalar.split_at(split);
        let chunks = Chunks(vec![head, tail].into_iter());
        let bytes = runtime().block_on(drain(Some(chunks), true)).unwrap();
        assert_eq!(
            String::from_utf8_lossy(&bytes),
            "\u{10348}",
            "split {split}"
        );
    }
}

#[test]
fn discarded_pipe_returns_no_bytes() {
    let chunks = Chunks(vec![&b"lost"[..]].into_iter());
    assert_eq!(runtime().block_on(drain(Some(chunks), false)).unwrap(), b"");
}

#[test]
fn pipe_failures_preserve_the_original_io_error() {
    for keep in [true, false] {
        let error = runtime().block_on(drain(Some(Failing), keep)).unwrap_err();
        assert!(error.into_inner().unwrap().is::<Marker>(), "{keep}");
    }
}

/// Control files that gate a shell and the background reader it leaves behind.
struct Held {
    /// The shell, not yet reaped.
    child: CapturedChild,
    /// The process id of the background reader holding both pipes.
    pid: String,
    /// Opening this file for writing lets the background reader end.
    hold: PathBuf,
    /// Opening this file for writing lets the shell exit.
    gate: PathBuf,
    /// Whether the background reader was released.
    released: bool,
}

impl Held {
    /// Lets the shell exit.
    fn open_gate(&self) {
        std::fs::write(&self.gate, b"").unwrap();
    }

    /// Lets the background reader end.
    fn release(&mut self) {
        std::fs::write(&self.hold, b"").unwrap();
        self.released = true;
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        if !self.released {
            let _ = std::process::Command::new("kill").arg(&self.pid).status();
        }
        let _ = std::fs::remove_file(&self.hold);
        let _ = std::fs::remove_file(&self.gate);
    }
}

/// A fresh named pipe in the temporary directory.
fn fifo(label: &str) -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let path = std::env::temp_dir().join(format!(
        "maestro-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    path
}

/// Starts a shell that writes `diag` to stderr, leaves a background reader holding
/// both pipes, reports the reader's process id and exits once its gate opens.
async fn held_shell() -> Held {
    let (hold, gate) = (fifo("hold"), fifo("gate"));
    let mut command = Command::new("sh");
    command
        .args([
            "-c",
            "echo diag >&2; read r < \"$1\" & echo $!; read g < \"$2\"; exit 0",
        ])
        .arg("sh")
        .arg(&hold)
        .arg(&gate)
        .kill_on_drop(true);
    let mut child = CapturedChild::spawn(&mut command).unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut pid = [0; 16];
    let mut length = 0;
    while !pid[..length].ends_with(b"\n") {
        length += stdout.read(&mut pid[length..]).await.unwrap();
    }
    child.stdout = Some(stdout);
    Held {
        child,
        pid: String::from_utf8(pid[..length - 1].to_vec()).unwrap(),
        hold,
        gate,
        released: false,
    }
}

/// Like [`held_shell`], with the shell already exited.
async fn held_pipes() -> Held {
    let mut held = held_shell().await;
    held.open_gate();
    held.child.child.wait().await.unwrap();
    held
}

#[test]
fn input_failure_remains_pending_until_output_settles() {
    runtime().block_on(async {
        let observed = Cell::new(false);
        let written = async {
            observed.set(true);
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        };
        let (release, held) = tokio::sync::oneshot::channel();
        let output = async {
            held.await.unwrap();
            Ok(std::process::Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: Vec::new(),
                stderr: Vec::new(),
            })
        };
        let mut settlement = Box::pin(super::settle_input(written, output));
        let pending = is_pending(&mut settlement).await;
        assert!(observed.get(), "input write has failed");
        assert!(pending, "input failure must wait for output completion");
        release.send(()).unwrap();
        assert_eq!(
            settlement.await.unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
    });
}

#[test]
fn grace_deadline_begins_after_child_exit() {
    // Idle paused-clock runtime jumps to the next armed timer: the wait ends exactly
    // when its grace timer, armed by the exit, fires.
    paused_runtime().block_on(async {
        for grace in [Duration::from_millis(100), Duration::from_millis(250)] {
            let mut held = held_shell().await;
            let options = WaitOptions {
                post_exit_grace: Some(grace),
                ..WaitOptions::default()
            };
            let gate = held.gate.clone();
            let mut wait = Box::pin(wait_for_child_process(
                &mut held.child,
                options,
                std::future::pending(),
                None,
            ));
            assert!(is_pending(&mut wait).await, "{grace:?} before the exit");
            tokio::time::advance(grace * 10).await;
            assert!(is_pending(&mut wait).await, "{grace:?} long after start");
            let exited = tokio::time::Instant::now();
            std::fs::write(&gate, b"").unwrap();
            let output = wait.await.unwrap();
            assert_eq!(exited.elapsed(), grace, "ended by the grace after the exit");
            assert!(output.status.success(), "{grace:?}");
            held.release();
        }
    });
}

#[test]
fn discard_drops_stderr_retained_by_an_earlier_dropped_wait() {
    paused_runtime().block_on(async {
        let mut held = held_pipes().await;
        let capture = wait_for_child_process(
            &mut held.child,
            WaitOptions::default(),
            std::future::pending(),
            None,
        );
        assert!(
            tokio::time::timeout(Duration::from_secs(1), capture)
                .await
                .is_err(),
            "held pipes keep the capture wait pending"
        );
        let options = WaitOptions {
            stderr: OutputMode::Discard,
            post_exit_grace: Some(Duration::from_millis(10)),
        };
        let output = wait_for_child_process(&mut held.child, options, std::future::pending(), None)
            .await
            .unwrap();
        assert_eq!(output.stderr, b"");
        held.release();
    });
}

#[test]
fn ready_termination_is_observed_even_when_the_child_already_finished() {
    runtime().block_on(async {
        let mut command = Command::new("sh");
        command.args(["-c", "echo out; echo err >&2"]);
        let mut child = CapturedChild::spawn(&mut command).unwrap();
        child.child.wait().await.unwrap();
        let polled = Rc::new(Cell::new(false));
        let seen = polled.clone();
        let stop = async move {
            seen.set(true);
            Termination::Terminate
        };
        let output = wait_for_child_process(&mut child, WaitOptions::default(), stop, None)
            .await
            .unwrap();
        assert!(polled.get(), "termination polled before completion");
        assert_eq!(
            (output.stdout, output.stderr),
            (b"out\n".to_vec(), b"err\n".to_vec())
        );
    });
}

#[test]
fn termination_is_polled_while_a_stream_stays_ready() {
    runtime().block_on(async {
        let mut command = Command::new("yes");
        command.kill_on_drop(true);
        let mut child = CapturedChild::spawn(&mut command).unwrap();
        let polls = Rc::new(Cell::new(0));
        let counter = polls.clone();
        let stop = std::future::poll_fn(move |cx| {
            counter.set(counter.get() + 1);
            if counter.get() < 50 {
                cx.waker().wake_by_ref();
                Poll::Pending
            } else {
                Poll::Ready(Termination::Kill)
            }
        });
        let output = wait_for_child_process(&mut child, WaitOptions::default(), stop, None)
            .await
            .unwrap();
        assert_eq!(output.status.signal(), Some(9));
        assert_eq!(output.stdout, b"", "abandoned stream is empty");
    });
}

/// Records its drop in a shared flag; the timers of a wait are locals of
/// [`wait_for_child_process`] and drop with the same frame, which ownership enforces.
struct Observed(Rc<Cell<bool>>);
impl Future for Observed {
    type Output = Termination;
    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Termination> {
        Poll::Pending
    }
}
impl Drop for Observed {
    fn drop(&mut self) {
        self.0.set(true);
    }
}

#[test]
fn settled_wait_releases_timers_and_stop_future() {
    paused_runtime().block_on(async {
        let dropped = Rc::new(Cell::new(false));
        let mut child = CapturedChild::spawn(&mut Command::new("true")).unwrap();
        wait_for_child_process(
            &mut child,
            WaitOptions::default(),
            Observed(dropped.clone()),
            None,
        )
        .await
        .unwrap();
        assert!(dropped.get(), "end of file");

        let dropped = Rc::new(Cell::new(false));
        let mut held = held_pipes().await;
        let options = WaitOptions {
            post_exit_grace: Some(Duration::from_millis(100)),
            ..WaitOptions::default()
        };
        wait_for_child_process(&mut held.child, options, Observed(dropped.clone()), None)
            .await
            .unwrap();
        assert!(dropped.get(), "grace expiry");
        held.release();

        let dropped = Rc::new(Cell::new(false));
        let mut child = CapturedChild::spawn(&mut Command::new("true")).unwrap();
        child.child.wait().await.unwrap();
        child.stderr = Some(unreadable_stderr().await);
        wait_for_child_process(
            &mut child,
            WaitOptions::default(),
            Observed(dropped.clone()),
            None,
        )
        .await
        .unwrap_err();
        assert!(dropped.get(), "pipe failure");
    });
}

/// A stderr whose reads fail with a connection reset: a socket whose peer reset it.
async fn unreadable_stderr() -> tokio::process::ChildStderr {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let client = tokio::net::TcpStream::connect(listener.local_addr().unwrap())
        .await
        .unwrap();
    let (server, _) = listener.accept().await.unwrap();
    server.set_zero_linger().unwrap();
    drop(server);
    let stderr =
        std::process::ChildStderr::from(std::os::fd::OwnedFd::from(client.into_std().unwrap()));
    tokio::process::ChildStderr::from_std(stderr).unwrap()
}

#[test]
fn failed_wait_leaves_consumed_bytes_available() {
    runtime().block_on(async {
        let mut command = Command::new("printf");
        command.arg("prefix");
        let mut child = CapturedChild::spawn(&mut command).unwrap();
        child.child.wait().await.unwrap();
        child.stderr = Some(unreadable_stderr().await);
        let error = wait_for_child_process(
            &mut child,
            WaitOptions::default(),
            std::future::pending(),
            None,
        )
        .await
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::ConnectionReset);
        assert!(child.stderr.is_none(), "failed pipe closed");
        assert_eq!(child.take_output(), (b"prefix".to_vec(), vec![]));
        assert_eq!(child.take_output(), (vec![], vec![]));
    });
}

#[test]
fn streamed_failure_preserves_prior_bytes_without_replay() {
    runtime().block_on(async {
        let mut command = Command::new("printf");
        command.arg("new-bytes");
        let mut child = CapturedChild::spawn(&mut command).unwrap();
        child.child.wait().await.unwrap();
        child.out.extend_from_slice(b"prior-out");
        child.err.extend_from_slice(b"prior-err");
        child.stderr = Some(unreadable_stderr().await);
        let mut delivered = Vec::new();
        let mut sink = |_, data: &[u8]| delivered.extend_from_slice(data);
        let error = wait_for_child_process(
            &mut child,
            WaitOptions::default(),
            std::future::pending(),
            Some(&mut sink),
        )
        .await
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::ConnectionReset);
        let output = wait_for_child_process(
            &mut child,
            WaitOptions::default(),
            std::future::pending(),
            Some(&mut sink),
        )
        .await
        .unwrap();
        assert_eq!(delivered, b"new-bytes");
        assert_eq!(output.stdout, b"prior-out");
        assert_eq!(output.stderr, b"prior-err");
    });
}

/// A [`children::stubborn`] command connecting back to a fresh listener, with `variables`
/// added to its environment.
fn stubborn_command_with(variables: &[(&str, &str)]) -> (Command, std::net::TcpListener) {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let mut all = vec![("CHILD_PORT", port.as_str())];
    all.extend_from_slice(variables);
    let mut command = async_command(children::stubborn, &all);
    command.kill_on_drop(true);
    (command, listener)
}

/// A [`children::stubborn`] command connecting back to a fresh listener.
fn stubborn_command() -> (Command, std::net::TcpListener) {
    stubborn_command_with(&[])
}

/// Blocks until a [`children::stubborn`] child reports `READY`, returning its socket.
fn stubborn_ready(listener: &std::net::TcpListener) -> std::net::TcpStream {
    let (mut socket, _) = listener.accept().unwrap();
    socket.read_exact(&mut [0; 5]).unwrap();
    socket
}

/// Starts a [`children::stubborn`] child and returns it with its connected socket.
fn stubborn() -> (CapturedChild, std::net::TcpStream) {
    let (mut command, listener) = stubborn_command();
    let child = CapturedChild::spawn(&mut command).unwrap();
    (child, stubborn_ready(&listener))
}

/// Blocks until a [`children::stubborn`] child answers a termination signal it received.
fn read_signal_answer(socket: &mut std::net::TcpStream) {
    let mut answer = [0; 1];
    socket.read_exact(&mut answer).unwrap();
    assert_eq!(&answer, b"T");
}

#[test]
fn escalation_checks_live_state_before_forced_kill() {
    paused_runtime().block_on(async {
        let (mut live, mut socket) = stubborn();
        let after = Duration::from_secs(5);
        {
            let terminate = live.terminate(Termination::TerminateThenKill { after });
            tokio::pin!(terminate);
            assert!(is_pending(&mut terminate).await);
            read_signal_answer(&mut socket);
            tokio::time::advance(after.checked_sub(Duration::from_millis(1)).unwrap()).await;
            assert!(is_pending(&mut terminate).await, "running before the delay");
            tokio::time::advance(Duration::from_millis(1)).await;
            terminate.await.unwrap();
        }
        assert!(
            live.child.try_wait().unwrap().is_some(),
            "killed and reaped"
        );
        assert!(
            socket.read_exact(&mut [0; 1]).is_err(),
            "the killed child's end closed"
        );

        let mut exited = CapturedChild::spawn(&mut Command::new("true")).unwrap();
        exited.child.wait().await.unwrap();
        let terminate = exited.terminate(Termination::TerminateThenKill { after });
        tokio::pin!(terminate);
        assert!(
            !is_pending(&mut terminate).await,
            "exited child is not waited on"
        );
    });
}

#[test]
fn termination_requests_signal_only_a_running_child() {
    runtime().block_on(async {
        let mut reaped = CapturedChild::spawn(&mut Command::new("true")).unwrap();
        reaped.child.wait().await.unwrap();
        assert!(
            !reaped.request_termination(),
            "a reaped child is not signalled"
        );

        let (mut running, mut socket) = stubborn();
        assert!(
            running.request_termination(),
            "a running child is signalled"
        );
        read_signal_answer(&mut socket);
        running.terminate(Termination::Kill).await.unwrap();
    });
}

#[test]
fn wait_escalation_timer_kills_exactly_after_the_selected_delay() {
    // Idle paused-clock runtime jumps to the next armed timer: the wait ends exactly
    // when the escalation timer, armed by the request, fires.
    paused_runtime().block_on(async {
        let (mut live, _socket) = stubborn();
        let after = Duration::from_secs(5);
        let started = tokio::time::Instant::now();
        let stop = std::future::ready(Termination::TerminateThenKill { after });
        let output = wait_for_child_process(&mut live, WaitOptions::default(), stop, None)
            .await
            .unwrap();
        assert_eq!(output.status.signal(), Some(9));
        assert_eq!(started.elapsed(), after);
    });
}

#[test]
fn wait_consumes_the_termination_decision_once() {
    runtime().block_on(async {
        let mut command = Command::new("sh");
        command.args(["-c", "exec sleep 60"]).kill_on_drop(true);
        let mut child = CapturedChild::spawn(&mut command).unwrap();
        let polled = Rc::new(Cell::new(false));
        let seen = polled.clone();
        let stop = std::future::poll_fn(move |_| {
            assert!(!seen.replace(true), "decision polled after it was consumed");
            Poll::Ready(Termination::Terminate)
        });
        let output = wait_for_child_process(&mut child, WaitOptions::default(), stop, None)
            .await
            .unwrap();
        assert_eq!(output.status.signal(), Some(15));
        assert!(polled.get());
    });
}

#[test]
fn sync_capture_uses_each_callers_timeout() {
    // The blocking readiness read holds the only runtime thread, so the paused
    // clock cannot reach the deadline before the child ignores termination.
    paused_runtime().block_on(async {
        for timeout in [Duration::from_millis(5000), Duration::from_millis(10_000)] {
            let (mut command, listener) = stubborn_command();
            let started = tokio::time::Instant::now();
            let mut run = Box::pin(capture(
                &mut command,
                SpawnSyncOptions::Capture {
                    max_buffer_bytes: None,
                    stdout: OutputMode::Capture,
                    input: None,
                    timeout: Some(timeout),
                    stderr: OutputMode::Discard,
                },
            ));
            assert!(is_pending(&mut run).await, "{timeout:?} started");
            let mut socket = stubborn_ready(&listener);
            assert!(
                matches!(run.await, Err(SpawnSyncError::Timeout)),
                "{timeout:?}"
            );
            assert_eq!(started.elapsed(), timeout);
            let mut rest = Vec::new();
            socket.read_to_end(&mut rest).unwrap();
            assert_eq!(
                rest, b"",
                "{timeout:?} killed without a termination request"
            );
        }
    });
}

#[test]
fn capture_timeout_closes_inherited_stdout_after_direct_child_exit() {
    use std::{error::Error, io::Write};
    const CASE: &str =
        "child_process::tests::capture_timeout_closes_inherited_stdout_after_direct_child_exit";
    if !maestro_test_process::selection().is_child() {
        let output = maestro_test_process::TestProcess::current(CASE)
            .unwrap()
            .nocapture()
            .case("*")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();

    paused_runtime().block_on(async {
        let port = listener.local_addr().unwrap().port().to_string();
        let mut command = async_command(children::inherited_output, &[("CHILD_PORT", &port)]);
        let timeout = Duration::from_secs(5);
        let started = tokio::time::Instant::now();
        let mut run = Box::pin(capture(
            &mut command,
            SpawnSyncOptions::Capture {
                max_buffer_bytes: None,
                stdout: OutputMode::Capture,
                input: None,
                timeout: Some(timeout),
                stderr: OutputMode::Capture,
            },
        ));
        assert!(is_pending(&mut run).await, "capture started");
        // The holder connects only after the exit pipe reaches EOF. This blocking
        // accept keeps the paused runtime from advancing during either startup.
        let (mut holder, _) = listener.accept().unwrap();
        assert_eq!(started.elapsed(), Duration::ZERO);
        assert!(is_pending(&mut run).await, "stdout still held after exit");
        tokio::time::advance(timeout).await;
        let result = run.await;
        // Release before assertions so a wrong result cannot strand the holder.
        holder.write_all(b"!").unwrap();
        let mut report = Vec::new();
        holder.read_to_end(&mut report).unwrap();
        let error = result.unwrap_err();
        assert!(matches!(error, SpawnSyncError::Timeout));
        assert!(error.source().is_none());
        assert_eq!(error.to_string(), "The process timed out");
        assert_eq!(report, b"closedclosed");
        assert_eq!(started.elapsed(), timeout);
    });
}

#[test]
fn linked_descendant_keeps_its_link_until_the_parent_ends() {
    use std::io::{BufRead, BufReader, Write};
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let mut command = children::command(children::link_holder, &[("CHILD_PORT", &port)]);
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let collected = collect_output(command.spawn().unwrap(), &listener).unwrap();

    let (holder, _) = listener.accept().unwrap();
    let (mut probe, _) = listener.accept().unwrap();
    let mut holder = BufReader::new(holder);
    let mut line = String::new();
    holder.read_line(&mut line).unwrap();
    assert_eq!(line, "linked\n");
    probe.write_all(b"p").unwrap();
    let mut state = [0];
    probe.read_exact(&mut state).unwrap();
    // Release before assertions so a wrong result cannot strand the holder.
    holder.get_mut().write_all(b"!").unwrap();
    let output = collected.join().unwrap().unwrap();

    assert_eq!(&state, b"H");
    assert!(output.status.success());
}

#[test]
fn stream_callback_can_abort_the_same_wait() {
    runtime().block_on(async {
        let mut command = async_command(children::prefix_then_wait, &[]);
        let mut child = CapturedChild::spawn(&mut command).unwrap();
        let (tx, rx) = tokio::sync::oneshot::channel();
        let mut tx = Some(tx);
        let mut bytes = Vec::new();
        let mut sink = |_, chunk: &[u8]| {
            bytes.extend_from_slice(chunk);
            if bytes.ends_with(b"prefix") {
                tx.take().unwrap().send(()).unwrap();
            }
        };
        let output = wait_for_child_process(
            &mut child,
            WaitOptions::default(),
            async {
                rx.await.unwrap();
                Termination::Terminate
            },
            Some(&mut sink),
        )
        .await
        .unwrap();
        assert_eq!(bytes, harness(b"prefix"));
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    });
}

#[test]
fn capture_overflow_drops_inherited_stdin() {
    use std::io::Write;
    runtime().block_on(async {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port().to_string();
        let mut command = async_command(
            children::input_hold,
            &[
                ("CHILD_PORT", &port),
                ("CHILD_EXITED", "1"),
                ("CHILD_OVERFLOW", "1"),
            ],
        );
        // The orphan holds stdin without reading it; its readiness follows parent exit.
        let input = vec![b'x'; 4_194_304];
        let timeout = Duration::from_secs(5);
        // Parent and orphan each write the start prologue; past both, only the released orphan's
        // finished write trips the limit, so the capture never closes a pipe the orphan writes.
        let mut run = Box::pin(capture(
            &mut command,
            SpawnSyncOptions::Capture {
                max_buffer_bytes: Some(2 * HARNESS_START.len()),
                stdout: OutputMode::Capture,
                input: Some(&input),
                timeout: Some(timeout),
                stderr: OutputMode::Capture,
            },
        ));
        assert!(is_pending(&mut run).await);
        let (mut socket, _) = listener.accept().unwrap();
        let mut pid = String::new();
        std::io::BufRead::read_line(&mut std::io::BufReader::new(&socket), &mut pid).unwrap();
        tokio::task::yield_now().await;
        assert!(
            is_pending(&mut run).await,
            "both start prologues stay within the limit"
        );
        socket.write_all(b"!").unwrap();
        let result = run.await;
        socket.write_all(b"!").unwrap();
        let mut rest = [0; 4];
        socket.read_exact(&mut rest).unwrap();
        assert_eq!(&rest, b"done");
        let error = result.unwrap_err();
        let SpawnSyncError::Wait(error) = error else {
            panic!("expected output-limit failure, got {error:?}");
        };
        assert_eq!(error.to_string(), "captured output exceeds byte limit");
    });
}

/// A [`children::input_hold`] command that connects to `port` and, when `exited`, leaves
/// its holder behind after ending.
fn held_input_command(port: &str, exited: bool) -> Command {
    let mut variables = vec![("CHILD_PORT", port)];
    if exited {
        variables.push(("CHILD_EXITED", "1"));
    }
    async_command(children::input_hold, &variables)
}

#[test]
fn capture_input_deadline_reaps_the_ready_child() {
    paused_runtime().block_on(async {
        for exited in [false, true] {
            for timeout in [Duration::from_secs(5), Duration::from_secs(10)] {
                let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();

                let port = listener.local_addr().unwrap().port().to_string();
                let mut command = held_input_command(&port, exited);
                let input = vec![b'x'; 4_194_304];
                let mut run = Box::pin(capture(
                    &mut command,
                    SpawnSyncOptions::Capture {
                        max_buffer_bytes: None,
                        stdout: OutputMode::Capture,
                        input: Some(&input),
                        timeout: Some(timeout),
                        stderr: OutputMode::Capture,
                    },
                ));
                assert!(is_pending(&mut run).await);
                let (mut socket, _) = listener.accept().unwrap();
                let mut pid = String::new();
                std::io::BufRead::read_line(&mut std::io::BufReader::new(&socket), &mut pid)
                    .unwrap();
                let pid = rustix::process::Pid::from_raw(pid.trim().parse().unwrap()).unwrap();
                tokio::task::yield_now().await;
                assert!(
                    is_pending(&mut run).await,
                    "input remains incomplete before deadline"
                );
                tokio::time::advance(timeout).await;
                let result = run.await;
                release_exited_holder(&mut socket, exited);
                assert!(matches!(result, Err(SpawnSyncError::Timeout)));
                assert_eq!(
                    rustix::process::waitpid(Some(pid), rustix::process::WaitOptions::NOHANG)
                        .unwrap_err(),
                    rustix::io::Errno::CHILD
                );
                let mut rest = Vec::new();
                socket.read_to_end(&mut rest).unwrap();
                assert!(rest.is_empty());
            }
        }
    });
}

#[test]
fn capture_without_limit_waits_for_the_release() {
    use std::io::Write;
    paused_runtime().block_on(async {
        for timeout in [None, Some(Duration::ZERO)] {
            let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
            let port = listener.local_addr().unwrap().port().to_string();
            let mut command = async_command(
                children::stubborn,
                &[("CHILD_PORT", &port), ("CHILD_AFTER", "echo")],
            );
            let mut run = Box::pin(capture(
                &mut command,
                SpawnSyncOptions::Capture {
                    max_buffer_bytes: None,
                    stdout: OutputMode::Capture,
                    input: Some(b"input"),
                    timeout,
                    stderr: OutputMode::Capture,
                },
            ));
            assert!(is_pending(&mut run).await);
            let mut socket = stubborn_ready(&listener);
            tokio::time::advance(Duration::from_secs(100)).await;
            assert!(is_pending(&mut run).await, "no deadline before release");
            socket.write_all(b"!").unwrap();
            let output = run.await.unwrap();
            assert!(output.status.success());
            assert_eq!(output.stdout, harness(b"input complete"));
            assert_eq!(output.stderr, b"diag");
        }
    });
}

/// Release a surviving descendant; live direct children have already been killed.
fn release_exited_holder(socket: &mut std::net::TcpStream, exited: bool) {
    use std::io::Write;
    if exited {
        socket.write_all(b"!").unwrap();
    }
}

#[test]
#[cfg(unix)]
fn signal_lists_share_native_signal_validation() {
    use super::{Signal, process_exists, signal_processes};
    use std::{
        io::{BufRead, BufReader},
        os::unix::process::ExitStatusExt,
        process::Stdio,
    };
    if maestro_test_process::run_alone("signal_lists_share_native_signal_validation").unwrap() {
        return;
    }
    assert!(!process_exists(0));
    assert!(!process_exists(u32::MAX));
    assert!(process_exists(std::process::id()));
    assert!(super::native::exists_from_probe(Err(
        rustix::io::Errno::PERM
    )));
    assert!(!super::native::exists_from_probe(Err(
        rustix::io::Errno::SRCH
    )));
    for (signal, number) in [
        (Signal::Hangup, 1),
        (Signal::Terminate, 15),
        (Signal::Kill, 9),
    ] {
        let mut child = children::command(children::ready_then_wait, &[])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let mut marker = String::new();
        while marker != "ready\n" {
            marker.clear();
            assert_ne!(output.read_line(&mut marker).unwrap(), 0);
        }
        assert!(process_exists(child.id()));
        signal_processes(&[0, u32::MAX, i32::MAX.unsigned_abs(), child.id()], signal);
        assert_eq!(child.wait().unwrap().signal(), Some(number));
        assert!(!process_exists(child.id()));
    }
}
