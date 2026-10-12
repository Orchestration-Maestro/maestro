//! Private byte-collector behaviour.
use super::{StderrMode, collect};
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, ReadBuf};

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

/// A single-threaded runtime for the collector.
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
}

#[test]
fn bytes_split_across_consumed_reads_decode_as_one_character() {
    let chunks = Chunks(vec![&b"\xE2"[..], &b"\x82\xAC"[..]].into_iter());
    let bytes = runtime()
        .block_on(collect(Some(chunks), StderrMode::Capture))
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&bytes), "€");
}

#[test]
fn pipe_failures_preserve_the_original_io_error() {
    for mode in [StderrMode::Capture, StderrMode::Discard] {
        let error = runtime()
            .block_on(collect(Some(Failing), mode))
            .unwrap_err();
        assert!(error.into_inner().unwrap().is::<Marker>(), "{mode:?}");
    }
}
