//! Private collector, grace and escalation behaviour.
use super::{CapturedChild, Termination, WaitOptions, fill, wait_for_child_process};
use std::{
    cell::Cell,
    future::Future,
    io,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
    time::Duration,
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt, ReadBuf};
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

/// A reader that yields one chunk and then fails with the marker.
struct ThenFailing(Option<&'static [u8]>);
impl AsyncRead for ThenFailing {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.0.take() {
            Some(chunk) => {
                buf.put_slice(chunk);
                Poll::Ready(Ok(()))
            }
            None => Poll::Ready(Err(io::Error::other(Marker))),
        }
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

#[test]
fn wait_failure_leaves_consumed_bytes_available() {
    runtime().block_on(async {
        let mut pipe = Some(ThenFailing(Some(b"prefix")));
        let mut buf = Vec::new();
        fill(&mut pipe, &mut buf, true).await.unwrap();
        let error = fill(&mut pipe, &mut buf, true).await.unwrap_err();
        assert!(error.into_inner().unwrap().is::<Marker>());
        assert!(pipe.is_none(), "failed pipe closed");
        let mut child = CapturedChild::spawn(&mut Command::new("true")).unwrap();
        child.out = buf;
        assert_eq!(child.take_output(), (b"prefix".to_vec(), vec![]));
        assert_eq!(child.take_output(), (vec![], vec![]));
    });
}

/// Starts a shell whose direct exit leaves a background sleep holding both pipes,
/// returning the child and the sleep's process id.
async fn held_pipes() -> (CapturedChild, String) {
    let mut command = Command::new("sh");
    command.args(["-c", "sleep 60 & echo $!; exit 0"]);
    let mut child = CapturedChild::spawn(&mut command).unwrap();
    child.child.wait().await.unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut pid = [0; 16];
    let mut length = 0;
    while !pid[..length].ends_with(b"\n") {
        length += stdout.read(&mut pid[length..]).await.unwrap();
    }
    child.stdout = Some(stdout);
    (
        child,
        String::from_utf8(pid[..length - 1].to_vec()).unwrap(),
    )
}

/// Ends the background sleep started by [`held_pipes`].
fn release_held(pid: &str) {
    assert!(
        std::process::Command::new("kill")
            .arg(pid)
            .status()
            .unwrap()
            .success()
    );
}

/// Polls `future` once and reports whether it is still pending.
async fn is_pending<F: Future + Unpin>(future: &mut F) -> bool {
    std::future::poll_fn(|cx| Poll::Ready(Pin::new(&mut *future).poll(cx).is_pending())).await
}

#[test]
fn grace_deadline_begins_after_child_exit() {
    paused_runtime().block_on(async {
        for grace in [Duration::from_millis(100), Duration::from_millis(250)] {
            let (mut child, pid) = held_pipes().await;
            let options = WaitOptions {
                post_exit_grace: Some(grace),
                ..WaitOptions::default()
            };
            let wait = wait_for_child_process(&mut child, options, std::future::pending());
            tokio::pin!(wait);
            assert!(is_pending(&mut wait).await, "{grace:?} at exit");
            tokio::time::advance(grace.checked_sub(Duration::from_millis(1)).unwrap()).await;
            assert!(is_pending(&mut wait).await, "{grace:?} before the deadline");
            tokio::time::advance(Duration::from_millis(1)).await;
            assert!(wait.await.unwrap().status.success(), "{grace:?}");
            release_held(&pid);
        }
    });
}

/// Records its drop in a shared flag.
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
        let (mut child, pid) = held_pipes().await;
        let options = WaitOptions {
            post_exit_grace: Some(Duration::from_millis(100)),
            ..WaitOptions::default()
        };
        wait_for_child_process(&mut child, options, Observed(dropped.clone()))
            .await
            .unwrap();
        assert!(dropped.get(), "grace expiry");
        release_held(&pid);
    });
}

/// A child that reports `READY`, then answers each termination request with `T` while
/// ignoring it.
const STUBBORN: &str = r"
import signal, socket, sys, time
s = socket.create_connection(('127.0.0.1', int(sys.argv[1])))
signal.signal(signal.SIGTERM, lambda *_: s.sendall(b'T'))
s.sendall(b'READY')
while True: time.sleep(1)
";

#[test]
fn escalation_checks_live_state_before_forced_kill() {
    paused_runtime().block_on(async {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let mut command = Command::new("python3");
        command.args([
            "-c",
            STUBBORN,
            &listener.local_addr().unwrap().port().to_string(),
        ]);
        let mut live = CapturedChild::spawn(&mut command).unwrap();
        let (mut socket, _) = listener.accept().await.unwrap();
        socket.read_exact(&mut [0; 5]).await.unwrap();
        let after = Duration::from_secs(5);
        {
            let terminate = live.terminate(Termination::TerminateThenKill { after });
            tokio::pin!(terminate);
            assert!(is_pending(&mut terminate).await);
            socket.read_exact(&mut [0; 1]).await.unwrap();
            tokio::time::advance(after.checked_sub(Duration::from_millis(1)).unwrap()).await;
            assert!(is_pending(&mut terminate).await, "running before the delay");
            tokio::time::advance(Duration::from_millis(1)).await;
            terminate.await.unwrap();
        }
        assert!(
            live.child.try_wait().unwrap().is_some(),
            "killed and reaped"
        );
        socket.shutdown().await.unwrap();

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
