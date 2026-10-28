//! The first-line deadline of a socket connection.
use super::Admission;
use maestro_app::rpc_mode::{RpcInput, RpcLineReader};
use std::{
    future::Future,
    io,
    pin::Pin,
    task::{Context, Poll, Waker},
    time::Duration,
};
use tokio::io::AsyncWriteExt;
use tokio::time::{Instant, advance};

/// The outcome of a read: a line, the end, or a failure's kind and text.
type Read = Result<RpcInput, (io::ErrorKind, String)>;

/// Polls `next` once, without a waker.
fn poll(next: &mut Pin<Box<dyn Future<Output = io::Result<RpcInput>> + '_>>) -> Poll<Read> {
    let polled = next.as_mut().poll(&mut Context::from_waker(Waker::noop()));
    polled.map(|read| read.map_err(|error| (error.kind(), error.to_string())))
}

/// Runs `test` on a current-thread runtime whose clock moves only when the test advances it.
fn paused(test: impl Future<Output = ()>) {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .start_paused(true)
        .build()
        .unwrap()
        .block_on(test);
}

/// The first line must complete within 5 s of admission, however recently a byte arrived; a
/// line already waiting when the deadline has passed still wins, and later lines have no
/// deadline.
#[test]
fn socket_initial_admission_keeps_its_own_deadline() {
    paused(async {
        let (mut client, server) = tokio::io::duplex(64);
        let start = Instant::now();
        let mut admission = Admission::new(server);
        let mut next = admission.next_line();
        advance(Duration::from_secs(3)).await;
        client.write_all(b"{\"id\":").await.unwrap();
        assert!(poll(&mut next).is_pending(), "the partial line was read");
        advance(Duration::from_millis(1999)).await;
        assert!(poll(&mut next).is_pending(), "just before the deadline");
        advance(Duration::from_millis(1)).await;
        let expired = (
            io::ErrorKind::TimedOut,
            "timed out reading api request".to_owned(),
        );
        assert_eq!(poll(&mut next), Poll::Ready(Err(expired)));
        assert_eq!(start.elapsed(), Duration::from_secs(5));

        let (mut client, server) = tokio::io::duplex(64);
        let mut admission = Admission::new(server);
        advance(Duration::from_secs(6)).await;
        client.write_all(b" {\"type\":\"ping\"} \n").await.unwrap();
        let mut next = admission.next_line();
        let ready = RpcInput::Line("{\"type\":\"ping\"}".into());
        assert_eq!(poll(&mut next), Poll::Ready(Ok(ready)));
        drop(next);
        let mut next = admission.next_line();
        advance(Duration::from_secs(60)).await;
        assert!(poll(&mut next).is_pending(), "later lines have no deadline");
        client.write_all(b"\n").await.unwrap();
        let blank = RpcInput::Line(String::new());
        assert_eq!(poll(&mut next), Poll::Ready(Ok(blank)));
    });
}
