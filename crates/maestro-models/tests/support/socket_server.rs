//! Loopback listener and complete WebSocket upgrade capture shared by test peers.

use std::sync::{Arc, Mutex, PoisonError};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{
    WebSocketStream, accept_hdr_async,
    tungstenite::handshake::server::{Callback, ErrorResponse, Request, Response},
};

/// Bind a loopback listener and name its endpoint.
pub(super) async fn listen() -> std::io::Result<(TcpListener, String)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("http://{}", listener.local_addr()?);
    Ok((listener, url))
}

/// Accept one socket and retain its complete upgrade request.
pub(super) async fn accept_request(
    listener: &TcpListener,
) -> Result<(WebSocketStream<TcpStream>, Request), Box<dyn std::error::Error + Send + Sync>> {
    let (stream, _) = listener.accept().await?;
    let seen = Arc::new(Mutex::new(None));
    let socket = accept_hdr_async(stream, Recorder(Arc::clone(&seen))).await?;
    let request = seen
        .lock()
        .map_err(|_| "upgrade poisoned")?
        .take()
        .ok_or("missing upgrade")?;
    Ok((socket, request))
}

/// Retains the upgrade seen by the native handshake implementation.
struct Recorder(Arc<Mutex<Option<Request>>>);

impl Callback for Recorder {
    fn on_request(self, request: &Request, response: Response) -> Result<Response, ErrorResponse> {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = Some(request.clone());
        Ok(response)
    }
}
