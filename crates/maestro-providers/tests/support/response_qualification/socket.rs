//! A finite native peer records the upgrade and response.create frame.

use super::{TestResult, Value};
use futures_util::{SinkExt, StreamExt};
use maestro_models::HttpRequest;
use tokio_tungstenite::tungstenite::Message;

/// Peer failures retain their native error; the caller joins after successful model observations.
type PeerResult = Result<HttpRequest, Box<dyn std::error::Error + Send + Sync>>;

/// Bind before dispatch; the returned task owns one finite peer exchange.
pub(crate) async fn serve(
    events: Vec<Value>,
) -> TestResult<(String, tokio::task::JoinHandle<PeerResult>)> {
    let (listener, base) = super::socket_server::listen().await?;
    let task = tokio::spawn(async move {
        let (mut socket, request) = super::socket_server::accept_request(&listener).await?;
        let frame = socket
            .next()
            .await
            .ok_or("missing socket frame")??
            .into_text()?;
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
