//! Captured native children observed through the public lifecycle interface.
#![cfg(test)]
#![cfg(unix)]
use crate::child_programs::{self, async_command};
use maestro_process::child_process::{
    CapturedChild, OutputMode, Termination, WaitOptions, wait_for_child_process,
};
use maestro_test_process::child::{harness, without_harness};
use std::cell::Cell;
use std::future::Future;
use std::os::unix::process::ExitStatusExt;
use std::pin::Pin;
use std::process::Output;
use std::rc::Rc;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// A runtime with the I/O driver the readiness sockets need.
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

/// A readiness listener and the child command that connects back to it.
async fn fixture(mode: &str, closed: &str) -> (TcpListener, tokio::process::Command) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let command = async_command(
        child_programs::held_child,
        &[
            ("CHILD_MODE", mode),
            ("CHILD_PORT", &port),
            ("CHILD_CLOSED", closed),
        ],
    );
    (listener, command)
}

/// Accepts one readiness connection after its `READY` greeting.
async fn ready(listener: &TcpListener) -> TcpStream {
    let (mut socket, _) = listener.accept().await.unwrap();
    let mut greeting = [0; 5];
    socket.read_exact(&mut greeting).await.unwrap();
    assert_eq!(&greeting, b"READY");
    socket
}

/// Releases a held child and reads its socket until the child closes it.
async fn release(mut socket: TcpStream) {
    let _ = socket.write_all(b"go").await;
    let mut rest = Vec::new();
    let _ = socket.read_to_end(&mut rest).await;
}

/// Polls `future` once and reports whether it is still pending.
async fn is_pending<F: Future + Unpin>(future: &mut F) -> bool {
    std::future::poll_fn(|cx| Poll::Ready(Pin::new(&mut *future).poll(cx).is_pending())).await
}

/// A stop future that never requests termination.
fn never() -> std::future::Pending<Termination> {
    std::future::pending()
}

/// Records its drop in a shared flag.
struct Observed<F> {
    /// The wrapped stop future.
    inner: Pin<Box<F>>,
    /// Set when the wrapper is dropped.
    dropped: Rc<Cell<bool>>,
}
impl<F: Future> Future for Observed<F> {
    type Output = F::Output;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<F::Output> {
        self.inner.as_mut().poll(cx)
    }
}
impl<F> Drop for Observed<F> {
    fn drop(&mut self) {
        self.dropped.set(true);
    }
}

/// Runs a held child, sends `requested` once every holder is ready, and returns the
/// output, the held sockets, the child and whether the stop future was dropped.
async fn request(requested: Termination) -> (Output, Vec<TcpStream>, CapturedChild, bool) {
    let mode = if requested == Termination::Kill {
        "kill"
    } else {
        "term"
    };
    let holders = if requested == Termination::Kill { 2 } else { 1 };
    let (listener, mut command) = fixture(mode, "").await;
    let mut child = CapturedChild::spawn(&mut command).unwrap();
    let (stop, request) = tokio::sync::oneshot::channel();
    let dropped = Rc::new(Cell::new(false));
    let observed = Observed {
        inner: Box::pin(async { request.await.unwrap() }),
        dropped: dropped.clone(),
    };
    let run = wait_for_child_process(&mut child, WaitOptions::default(), observed, None);
    let observe = async {
        let mut sockets = vec![];
        for _ in 0..holders {
            sockets.push(ready(&listener).await);
        }
        stop.send(requested).unwrap();
        sockets
    };
    let (output, sockets) = tokio::join!(run, observe);
    let released = dropped.get();
    (output.unwrap(), sockets, child, released)
}

#[test]
fn captured_process_keeps_native_command_and_raw_streams() {
    let dir = std::env::temp_dir().canonicalize().unwrap();
    for (mode, stderr) in [
        (OutputMode::Capture, Some(300_006)),
        (OutputMode::Discard, None),
    ] {
        let mut command = async_command(child_programs::raw_stream_writer, &[]);
        command
            .arg("arg value")
            .current_dir(&dir)
            .env("PROBE", "set");
        let output = runtime().block_on(async {
            let mut child = CapturedChild::spawn(&mut command).unwrap();
            wait_for_child_process(
                &mut child,
                WaitOptions {
                    stderr: mode,
                    ..WaitOptions::default()
                },
                never(),
                None,
            )
            .await
            .unwrap()
        });
        assert!(output.status.success());
        let mut expected = format!("arg value|{}|set", dir.display()).into_bytes();
        expected.extend_from_slice(b"\0\xe2");
        expected.extend(std::iter::repeat_n(b'o', 300_000));
        expected.extend_from_slice(b"\x82\xac\xff");
        assert_eq!(output.stdout, harness(&expected));
        match stderr {
            Some(length) => {
                assert_eq!(output.stderr.len(), length);
                assert_eq!(&output.stderr[..6], b"diag\0\xff");
            }
            None => assert_eq!(output.stderr, b""),
        }
    }
}

#[test]
fn exit_and_pipe_completion_orders_keep_output() {
    runtime().block_on(async {
        for (mode, closed, stdout, stderr) in [
            ("live", "", &b"earlytail"[..], &b"diagtail"[..]),
            ("live", "1", b"early", b"diagtail"),
            ("live", "2", b"earlytail", b"diag"),
            ("live", "12", b"early", b"diag"),
            ("exit", "", b"earlytail", b"diagtail"),
            ("exit", "1", b"early", b"diagtail"),
            ("exit", "2", b"earlytail", b"diag"),
        ] {
            let case = format!("{mode} closing {closed:?}");
            let (listener, mut command) = fixture(mode, closed).await;
            let mut child = CapturedChild::spawn(&mut command).unwrap();
            let run = wait_for_child_process(&mut child, WaitOptions::default(), never(), None);
            tokio::pin!(run);
            let socket = tokio::select! {
                socket = ready(&listener) => socket,
                _ = &mut run => panic!("{case} completed before the held producer closed"),
            };
            assert!(is_pending(&mut run).await, "{case}");
            let (output, ()) = tokio::join!(run, release(socket));
            let output = output.unwrap();
            assert!(output.status.success(), "{case}");
            assert_eq!(
                (without_harness(&output.stdout), output.stderr),
                (stdout.to_vec(), stderr.to_vec()),
                "{case}"
            );
        }
        let mut command = tokio::process::Command::new("true");
        let mut child = CapturedChild::spawn(&mut command).unwrap();
        let output = wait_for_child_process(&mut child, WaitOptions::default(), never(), None)
            .await
            .unwrap();
        assert!(output.status.success());
        assert_eq!((output.stdout, output.stderr), (vec![], vec![]));
        let again = wait_for_child_process(&mut child, WaitOptions::default(), never(), None)
            .await
            .unwrap();
        assert!(again.status.success());
        assert_eq!((again.stdout, again.stderr), (vec![], vec![]));
    });
}

#[test]
fn requested_termination_uses_live_state_and_reaps() {
    runtime().block_on(async {
        let (output, _sockets, _child, _) = request(Termination::Terminate).await;
        assert_eq!(without_harness(&output.stdout), b"terminated");
        assert!(output.status.success());

        let (output, sockets, mut child, _) = request(Termination::Kill).await;
        assert_eq!(output.status.signal(), Some(9));
        child.terminate(Termination::Kill).await.unwrap();
        child.terminate(Termination::Terminate).await.unwrap();
        for socket in sockets {
            release(socket).await;
        }
    });
}

#[test]
fn process_failures_preserve_native_error() {
    runtime().block_on(async {
        let mut missing = tokio::process::Command::new("maestro-missing-executable");
        let native = std::process::Command::new("maestro-missing-executable")
            .spawn()
            .unwrap_err();
        let error = CapturedChild::spawn(&mut missing).err().unwrap();
        assert_eq!(
            (error.kind(), error.raw_os_error()),
            (native.kind(), native.raw_os_error())
        );

        let mut cwd = tokio::process::Command::new("true");
        cwd.current_dir("/maestro-missing-directory");
        let native = std::process::Command::new("true")
            .current_dir("/maestro-missing-directory")
            .spawn()
            .unwrap_err();
        let error = CapturedChild::spawn(&mut cwd).err().unwrap();
        assert_eq!(
            (error.kind(), error.raw_os_error()),
            (native.kind(), native.raw_os_error())
        );

        let mut command = tokio::process::Command::new("true");
        let mut child = CapturedChild::spawn(&mut command).unwrap();
        wait_for_child_process(&mut child, WaitOptions::default(), never(), None)
            .await
            .unwrap();
        child.terminate(Termination::Terminate).await.unwrap();
        child.terminate(Termination::Kill).await.unwrap();
    });
}

#[test]
fn capture_driver_releases_completed_stop_future() {
    runtime().block_on(async {
        let dropped = Rc::new(Cell::new(false));
        let mut command = tokio::process::Command::new("true");
        let mut child = CapturedChild::spawn(&mut command).unwrap();
        let stop = Observed {
            inner: Box::pin(never()),
            dropped: dropped.clone(),
        };
        wait_for_child_process(&mut child, WaitOptions::default(), stop, None)
            .await
            .unwrap();
        assert!(dropped.get(), "normal completion");

        for requested in [Termination::Terminate, Termination::Kill] {
            let (_, sockets, _child, released) = request(requested).await;
            assert!(released, "{requested:?}");
            for socket in sockets {
                release(socket).await;
            }
        }
    });
}

/// Runs a held child with `options`, sends `requested` once its holder is ready and
/// returns the output with the still-held socket.
async fn run_held(
    mode: &str,
    closed: &str,
    options: WaitOptions,
    requested: Termination,
) -> (Output, TcpStream) {
    let (listener, mut command) = fixture(mode, closed).await;
    let mut child = CapturedChild::spawn(&mut command).unwrap();
    let (stop, request) = tokio::sync::oneshot::channel();
    let run = wait_for_child_process(&mut child, options, async { request.await.unwrap() }, None);
    let observe = async {
        let socket = ready(&listener).await;
        stop.send(requested).unwrap();
        socket
    };
    let (output, socket) = tokio::join!(run, observe);
    (output.unwrap(), socket)
}

#[test]
fn grace_preserves_prefixes_from_unfinished_streams() {
    runtime().block_on(async {
        for (closed, grace) in [
            ("", Duration::from_millis(100)),
            ("1", Duration::from_millis(100)),
            ("", Duration::ZERO),
        ] {
            let case = format!("closing {closed:?} with grace {grace:?}");
            let (listener, mut command) = fixture("exit", closed).await;
            let mut child = CapturedChild::spawn(&mut command).unwrap();
            let options = WaitOptions {
                post_exit_grace: Some(grace),
                ..WaitOptions::default()
            };
            let run = wait_for_child_process(&mut child, options, never(), None);
            let (output, socket) = tokio::join!(run, ready(&listener));
            let output = output.unwrap();
            assert!(output.status.success(), "{case}");
            if !grace.is_zero() {
                assert_eq!(
                    (without_harness(&output.stdout), output.stderr),
                    (b"early".to_vec(), b"diag".to_vec()),
                    "{case}"
                );
            }
            release(socket).await;
        }
    });
}

#[test]
fn termination_escalation_keeps_draining_until_exit() {
    runtime().block_on(async {
        let escalate = Termination::TerminateThenKill {
            after: Duration::from_secs(3600),
        };
        let (output, socket) = run_held("flood", "", WaitOptions::default(), escalate).await;
        assert!(output.status.success());
        assert_eq!(without_harness(&output.stdout).len(), 200_003);
        assert!(output.stdout.ends_with(b"xxend"));
        release(socket).await;
    });
}

#[test]
fn forced_kill_retains_only_completed_streams() {
    runtime().block_on(async {
        let (output, socket) =
            run_held("live", "1", WaitOptions::default(), Termination::Kill).await;
        assert_eq!(output.status.signal(), Some(9));
        assert_eq!(
            (without_harness(&output.stdout), output.stderr),
            (b"early".to_vec(), vec![])
        );
        release(socket).await;
    });
}
