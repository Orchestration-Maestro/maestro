//! Shared native upgrade and complete-message adapter.
use super::{ConnectWebSocket, WebSocket, WebSocketConnection, WebSocketFuture};
use crate::providers::transport::client_pairs;
use futures_util::{SinkExt as _, StreamExt as _};
use std::{fmt, io, sync::Arc};
use tokio_tungstenite::tungstenite::{
    Error, Message, client::IntoClientRequest, handshake::client::Request,
    protocol::WebSocketConfig,
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async_with_config};

/// Native socket retained by its caller's lifetime policy.
pub(crate) type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;
/// Open one upgrade without imposing message or frame size limits.
pub(crate) async fn upgrade(request: Request) -> Result<Socket, Error> {
    let config = WebSocketConfig::default()
        .max_message_size(None)
        .max_frame_size(None);
    connect_async_with_config(request, Some(config), false)
        .await
        .map(|(socket, _)| socket)
}
/// Preserve the library error itself in the standard I/O source chain.
#[derive(Debug)]
struct NativeCause(Error);
impl fmt::Display for NativeCause {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}
impl std::error::Error for NativeCause {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}
/// Wrap a native library error without losing its value or source.
fn native(error: Error) -> io::Error {
    io::Error::other(NativeCause(error))
}
/// Complete-message adapter whose socket is released on close or drop.
struct NativeSocket(Option<Socket>);
/// Report an operation on a released connection.
fn closed() -> io::Error {
    io::Error::new(io::ErrorKind::NotConnected, "WebSocket connection closed")
}
impl WebSocketConnection for NativeSocket {
    fn send(&mut self, bytes: Vec<u8>) -> WebSocketFuture<'_, io::Result<()>> {
        Box::pin(async move {
            let text = String::from_utf8(bytes).map_err(io::Error::other)?;
            self.0
                .as_mut()
                .ok_or_else(closed)?
                .send(Message::Text(text.into()))
                .await
                .map_err(native)
        })
    }
    fn receive(&mut self) -> WebSocketFuture<'_, io::Result<Option<Vec<u8>>>> {
        Box::pin(read(self.0.as_mut()))
    }

    fn close(&mut self) {
        self.0.take();
    }
}
/// Native connection factory using the shared upgrade primitive.
#[must_use]
pub fn default_web_socket() -> ConnectWebSocket {
    Arc::new(|mut request| {
        Box::pin(async move {
            let headers = client_pairs(&mut request.headers).map_err(io::Error::other)?;
            let mut native = request.url.into_client_request().map_err(native)?;
            native.headers_mut().extend(headers);
            Ok(Box::new(NativeSocket(Some(
                upgrade(native).await.map_err(self::native)?,
            ))) as WebSocket)
        })
    })
}

/// Skip native control traffic while retaining complete data frames.
async fn read(socket: Option<&mut Socket>) -> io::Result<Option<Vec<u8>>> {
    let socket = socket.ok_or_else(closed)?;
    while let Some(message) = socket.next().await {
        match message.map_err(native)? {
            Message::Text(text) => return Ok(Some(text.as_bytes().to_vec())),
            Message::Binary(bytes) => return Ok(Some(bytes.to_vec())),
            Message::Close(_) => return Ok(None),
            Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {}
        }
    }
    Ok(None)
}
