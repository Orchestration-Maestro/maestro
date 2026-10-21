//! Type-erased connected transports share the installed HTTP and TLS implementations.

use std::future::{Future, Ready, ready};
use std::pin::Pin;
use std::task::{Context, Poll};

use hyper::rt::{Read, ReadBufCursor, Write};
use hyper_util::client::legacy::connect::{Connected, Connection};
use tower_service::Service;

use super::Failure;

/// Native transport requirements for the request-owned HTTP driver.
trait Transport: Read + Write + Unpin + Send {}
impl<T: Read + Write + Unpin + Send> Transport for T {}

/// A TCP socket, TLS stream, or HTTP-upgraded tunnel, without another protocol codec.
pub(in crate::providers::http) struct Socket(Box<dyn Transport>);

impl Socket {
    /// Erase only the stream representation, never its ownership.
    pub(in crate::providers::http) fn new(
        stream: impl Read + Write + Unpin + Send + 'static,
    ) -> Self {
        Self(Box::new(stream))
    }
}

impl Read for Socket {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: ReadBufCursor<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut *self.0).poll_read(cx, buf)
    }
}

impl Write for Socket {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut *self.0).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut *self.0).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut *self.0).poll_shutdown(cx)
    }
}

impl Connection for Socket {
    fn connected(&self) -> Connected {
        Connected::new()
    }
}

/// One already-established socket is supplied to the shared TLS adapter.
struct Established(Option<Socket>);

impl Service<hyper::Uri> for Established {
    type Response = Socket;
    type Error = Failure;
    type Future = Ready<Result<Socket, Failure>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Failure>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: hyper::Uri) -> Self::Future {
        ready(
            self.0
                .take()
                .ok_or_else(|| std::io::Error::other("Socket already consumed").into()),
        )
    }
}

/// Apply the existing TLS connector to an established socket at the reached phase.
pub(in crate::providers::http) async fn secure(
    socket: Socket,
    uri: hyper::Uri,
    tls: rustls::ClientConfig,
) -> Result<Socket, Failure> {
    let mut connector = hyper_rustls::HttpsConnector::from((Established(Some(socket)), tls));
    Ok(Socket::new(connector.call(uri).await?))
}

/// Drive an owned HTTP exchange or an upgrade without detached tasks.
pub(in crate::providers::http) type Driver =
    Pin<Box<dyn Future<Output = Result<(), hyper::Error>> + Send>>;

/// Await an operation while the same owner drives its connection.
pub(in crate::providers::http) async fn driving<T>(
    connection: &mut Driver,
    operation: impl Future<Output = Result<T, hyper::Error>>,
) -> Result<T, Failure> {
    let mut operation = std::pin::pin!(operation);
    let mut completed = false;
    Ok(std::future::poll_fn(|cx| {
        if !completed {
            match connection.as_mut().poll(cx) {
                Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                Poll::Ready(Ok(())) => completed = true,
                Poll::Pending => {}
            }
        }
        operation.as_mut().poll(cx)
    })
    .await?)
}
