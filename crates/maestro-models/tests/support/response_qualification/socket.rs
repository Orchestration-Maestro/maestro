//! A finite native peer records the upgrade and response.create frame.

use super::{TestResult, Value};
use futures_util::{SinkExt, StreamExt};
use maestro_models::HttpRequest;
use std::sync::{Arc, Mutex, PoisonError};
use tokio::net::TcpListener;
use tokio_tungstenite::{
    accept_hdr_async,
    tungstenite::{
        Message,
        handshake::server::{Callback, ErrorResponse, Request, Response},
    },
};

/// Peer failures retain their native error; the caller joins after successful model observations.
type PeerResult = Result<HttpRequest, Box<dyn std::error::Error + Send + Sync>>;

/// Bind before dispatch; the returned task owns one finite peer exchange.
pub(super) async fn serve(
    events: Vec<Value>,
) -> TestResult<(String, tokio::task::JoinHandle<PeerResult>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let base = format!("http://{}", listener.local_addr()?);
    let task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        let seen = Arc::new(Mutex::new(None));
        let mut socket = accept_hdr_async(stream, Recorder(Arc::clone(&seen))).await?;
        let frame = socket
            .next()
            .await
            .ok_or("missing socket frame")??
            .into_text()?;
        let request = seen
            .lock()
            .map_err(|_| "upgrade poisoned")?
            .take()
            .ok_or("missing upgrade")?;
        for event in events {
            socket.send(Message::text(event.to_string())).await?;
        }
        socket.close(None).await?;
        Ok(HttpRequest {
            url: format!("ws://127.0.0.1:0{}", request.uri()),
            headers: request
                .headers()
                .iter()
                .map(|(k, v)| Ok((k.to_string(), v.to_str()?.to_owned())))
                .collect::<Result<_, tokio_tungstenite::tungstenite::http::header::ToStrError>>()?,
            body: frame.as_bytes().to_vec(),
            method: "GET".into(),
            signal: None,
        })
    });
    Ok((base, task))
}

/// Retains one actual upgrade without recreating handshake logic.
struct Recorder(Arc<Mutex<Option<Request>>>);

impl Callback for Recorder {
    fn on_request(self, request: &Request, response: Response) -> Result<Response, ErrorResponse> {
        let request = request.clone();
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = Some(request);
        Ok(response)
    }
}
