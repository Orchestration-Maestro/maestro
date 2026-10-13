//! Private collector, grace and escalation behaviour.
#![cfg(unix)]
use super::StderrMode;
use super::{CapturedChild, Termination, WaitOptions, fill, wait_for_child_process};
use std::io::{Read, Write};
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

/// A reader that hands the consumer exactly one chunk per read.
struct Chunks(std::vec::IntoIter<&'static [u8]>);
impl AsyncRead for Chunks {
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

/// A single-threaded runtime with I/O and timers.
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

/// A single-threaded runtime whose clock starts paused.
fn paused_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap()
}

#[test]
fn bytes_split_across_consumed_reads_decode_as_one_character() {
    let chunks = Chunks(vec![&b"\xE2"[..], &b"\x82\xAC"[..]].into_iter());
    let bytes = runtime().block_on(drain(Some(chunks), true)).unwrap();
    assert_eq!(String::from_utf8_lossy(&bytes), "€");
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

/// Polls `future` once and reports whether it is still pending.
async fn is_pending<F: Future + Unpin>(future: &mut F) -> bool {
    std::future::poll_fn(|cx| Poll::Ready(Pin::new(&mut *future).poll(cx).is_pending())).await
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
        );
        assert!(
            tokio::time::timeout(Duration::from_secs(1), capture)
                .await
                .is_err(),
            "held pipes keep the capture wait pending"
        );
        let options = WaitOptions {
            stderr: StderrMode::Discard,
            post_exit_grace: Some(Duration::from_millis(10)),
        };
        let output = wait_for_child_process(&mut held.child, options, std::future::pending())
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
        let output = wait_for_child_process(&mut child, WaitOptions::default(), stop)
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
        let output = wait_for_child_process(&mut child, WaitOptions::default(), stop)
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
        wait_for_child_process(&mut held.child, options, Observed(dropped.clone()))
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
        let error =
            wait_for_child_process(&mut child, WaitOptions::default(), std::future::pending())
                .await
                .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::ConnectionReset);
        assert!(child.stderr.is_none(), "failed pipe closed");
        assert_eq!(child.take_output(), (b"prefix".to_vec(), vec![]));
        assert_eq!(child.take_output(), (vec![], vec![]));
    });
}

/// A child that reports `READY`, answers each `p` with `E`, and answers each
/// termination request with `T` while ignoring it.
const STUBBORN: &str = r"
import signal, socket, sys
s = socket.create_connection(('127.0.0.1', int(sys.argv[1])))
signal.signal(signal.SIGTERM, lambda *_: s.sendall(b'T'))
s.sendall(b'READY')
while s.recv(1) == b'p': s.sendall(b'E')
";

/// Starts a [`STUBBORN`] child and returns it with its connected socket.
/// The socket blocks: a paused-clock runtime that yielded for I/O would jump to the
/// next timer.
fn stubborn() -> (CapturedChild, std::net::TcpStream) {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let mut command = Command::new("python3");
    command
        .args([
            "-c",
            STUBBORN,
            &listener.local_addr().unwrap().port().to_string(),
        ])
        .kill_on_drop(true);
    let child = CapturedChild::spawn(&mut command).unwrap();
    let (mut socket, _) = listener.accept().unwrap();
    socket.read_exact(&mut [0; 5]).unwrap();
    (child, socket)
}

/// Asks a [`STUBBORN`] child to answer and returns what it sent before the answer.
///
/// A termination request made before this call is queued before the child reads the
/// question, so its `T` precedes the answer.
fn sync(socket: &mut std::net::TcpStream) -> Vec<u8> {
    socket.write_all(b"p").unwrap();
    let mut sent = Vec::new();
    loop {
        let mut byte = [0; 1];
        socket.read_exact(&mut byte).unwrap();
        let byte = byte[0];
        if byte == b'E' {
            return sent;
        }
        sent.push(byte);
    }
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
            assert_eq!(sync(&mut socket), b"T", "requested once");
            tokio::time::advance(after.checked_sub(Duration::from_millis(1)).unwrap()).await;
            assert!(is_pending(&mut terminate).await, "running before the delay");
            assert_eq!(sync(&mut socket), b"", "still answering");
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
fn wait_escalation_timer_kills_exactly_after_the_selected_delay() {
    // Idle paused-clock runtime jumps to the next armed timer: the wait ends exactly
    // when the escalation timer, armed by the request, fires.
    paused_runtime().block_on(async {
        let (mut live, _socket) = stubborn();
        let after = Duration::from_secs(5);
        let started = tokio::time::Instant::now();
        let stop = std::future::ready(Termination::TerminateThenKill { after });
        let output = wait_for_child_process(&mut live, WaitOptions::default(), stop)
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
        let polls = Rc::new(Cell::new(0));
        let counter = polls.clone();
        let stop = std::future::poll_fn(move |_| {
            counter.set(counter.get() + 1);
            Poll::Ready(Termination::Terminate)
        });
        let output = wait_for_child_process(&mut child, WaitOptions::default(), stop)
            .await
            .unwrap();
        assert_eq!(output.status.signal(), Some(15));
        assert_eq!(polls.get(), 1, "polled once, after it was ready");
    });
}
