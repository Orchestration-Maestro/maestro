//! Replaceable complete-message socket transport.
use crate::BoxFuture;
use indexmap::IndexMap;
use std::{future::Future, io, pin::Pin, sync::Arc};

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::default_web_socket;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::{Socket, upgrade};

/// Owned destination and upgrade headers.
pub struct WebSocketRequest {
    /// Final socket URL.
    pub url: String,
    /// Caller-supplied upgrade headers.
    pub headers: IndexMap<String, String>,
}
/// One borrowed native socket operation.
#[cfg(not(target_arch = "wasm32"))]
pub type WebSocketFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
/// One borrowed browser-local socket operation.
#[cfg(target_arch = "wasm32")]
pub type WebSocketFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;
/// An owned complete-message connection.
pub trait WebSocketConnection {
    /// Send one UTF-8 text message.
    fn send(&mut self, bytes: Vec<u8>) -> WebSocketFuture<'_, io::Result<()>>;
    /// Receive one complete data message, or end of stream.
    fn receive(&mut self) -> WebSocketFuture<'_, io::Result<Option<Vec<u8>>>>;
    /// Release the connection without awaiting peer acknowledgement.
    fn close(&mut self);
}
/// Owned native connection.
#[cfg(not(target_arch = "wasm32"))]
pub type WebSocket = Box<dyn WebSocketConnection + Send>;
/// Owned browser-local connection.
#[cfg(target_arch = "wasm32")]
pub type WebSocket = Box<dyn WebSocketConnection>;
/// Replaceable native connection factory.
#[cfg(not(target_arch = "wasm32"))]
pub type ConnectWebSocket =
    Arc<dyn Fn(WebSocketRequest) -> BoxFuture<io::Result<WebSocket>> + Send + Sync>;
/// Replaceable browser-local connection factory.
#[cfg(target_arch = "wasm32")]
pub type ConnectWebSocket = Arc<dyn Fn(WebSocketRequest) -> BoxFuture<io::Result<WebSocket>>>;

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests;
