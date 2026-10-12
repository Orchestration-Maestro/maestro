//! Captured native children observed through the public lifecycle interface.
#![cfg(test)]
#![cfg(unix)]
use maestro_process::child_process::{
    CapturedChild, StderrMode, Termination, wait_for_child_process,
};
use std::cell::Cell;
use std::future::Future;
use std::os::unix::process::ExitStatusExt;
use std::pin::Pin;
use std::process::Output;
use std::rc::Rc;
use std::task::{Context, Poll};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Child behaviours selected by the first argument; later arguments are the port.
const CHILD: &str = r"
import os, signal, socket, sys, time
mode, port = sys.argv[1], int(sys.argv[2])
def hold():
    s = socket.create_connection(('127.0.0.1', port)); s.sendall(b'READY'); s.recv(1)
if mode == 'eof_first':
    os.write(1, b'early'); os.close(1); os.close(2); hold()
elif mode == 'exit_first':
    os.write(1, b'early'); os.write(2, b'diagnostic')
    parent = os.getpid()
    if os.fork() == 0:
        while os.getppid() == parent: time.sleep(0.001)
        hold(); os.write(1, b'tail'); os.write(2, b'late'); os._exit(0)
    os._exit(0)
elif mode == 'term':
    def stop(*_): os.write(1, b'terminated'); os._exit(0)
    signal.signal(signal.SIGTERM, stop); hold()
elif mode == 'kill':
    if os.fork() == 0: hold(); os._exit(0)
    hold()
";

/// A runtime with the I/O driver the readiness sockets need.
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

/// A readiness listener and the child command that connects back to it.
async fn fixture(mode: &str) -> (TcpListener, tokio::process::Command) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let mut command = tokio::process::Command::new("python3");
    command.args([
        "-c",
        CHILD,
        mode,
        &listener.local_addr().unwrap().port().to_string(),
    ]);
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

/// Releases a held child and waits until it closes its socket by exiting.
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
    let (listener, mut command) = fixture(mode).await;
    let mut child = CapturedChild::spawn(&mut command).unwrap();
    let (stop, request) = tokio::sync::oneshot::channel();
    let dropped = Rc::new(Cell::new(false));
    let observed = Observed {
        inner: Box::pin(async { request.await.unwrap() }),
        dropped: dropped.clone(),
    };
    let run = wait_for_child_process(&mut child, StderrMode::Capture, observed);
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
    let script = r"
import os, sys
assert sys.stdin.buffer.read() == b''
sys.stderr.buffer.write(b'diag\0\xff' + b'e' * 300000); sys.stderr.flush()
sys.stdout.buffer.write(sys.argv[1].encode() + b'|' + os.getcwd().encode() + b'|' + os.environ['PROBE'].encode() + b'\0\xe2')
sys.stdout.buffer.write(b'o' * 300000 + b'\x82\xac\xff')
";
    for (mode, stderr) in [
        (StderrMode::Capture, Some(300_006)),
        (StderrMode::Discard, None),
    ] {
        let mut command = tokio::process::Command::new("python3");
        command
            .args(["-c", script, "arg value"])
            .current_dir(&dir)
            .env("PROBE", "set");
        let output = runtime().block_on(async {
            let mut child = CapturedChild::spawn(&mut command).unwrap();
            wait_for_child_process(&mut child, mode, never())
                .await
                .unwrap()
        });
        assert!(output.status.success());
        let mut expected = format!("arg value|{}|set", dir.display()).into_bytes();
        expected.extend_from_slice(b"\0\xe2");
        expected.extend(std::iter::repeat_n(b'o', 300_000));
        expected.extend_from_slice(b"\x82\xac\xff");
        assert_eq!(output.stdout, expected);
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
fn captured_process_waits_for_exit_and_both_eofs() {
    runtime().block_on(async {
        for (mode, stdout, stderr) in [
            ("exit_first", &b"earlytail"[..], &b"diagnosticlate"[..]),
            ("eof_first", b"early", b""),
        ] {
            let (listener, mut command) = fixture(mode).await;
            let mut child = CapturedChild::spawn(&mut command).unwrap();
            let run = wait_for_child_process(&mut child, StderrMode::Capture, never());
            tokio::pin!(run);
            let socket = tokio::select! {
                socket = ready(&listener) => socket,
                _ = &mut run => panic!("{mode} completed before the held producer closed"),
            };
            assert!(is_pending(&mut run).await, "{mode}");
            let (output, ()) = tokio::join!(run, release(socket));
            let output = output.unwrap();
            assert!(output.status.success(), "{mode}");
            assert_eq!(
                (output.stdout, output.stderr),
                (stdout.to_vec(), stderr.to_vec()),
                "{mode}"
            );
        }
        let mut command = tokio::process::Command::new("true");
        let mut child = CapturedChild::spawn(&mut command).unwrap();
        let output = wait_for_child_process(&mut child, StderrMode::Capture, never())
            .await
            .unwrap();
        assert!(output.status.success());
        assert_eq!((output.stdout, output.stderr), (vec![], vec![]));
    });
}

#[test]
fn requested_termination_uses_live_state_and_reaps() {
    runtime().block_on(async {
        let (output, _sockets, _child, _) = request(Termination::Terminate).await;
        assert_eq!(output.stdout, b"terminated");
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
        wait_for_child_process(&mut child, StderrMode::Capture, never())
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
        wait_for_child_process(&mut child, StderrMode::Capture, stop)
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
