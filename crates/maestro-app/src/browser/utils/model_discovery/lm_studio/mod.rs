//! Downloaded model discovery through authenticated system RPC.
mod schema;
mod wire;
use super::{
    DiscoveryError, DiscoveryErrorKind, DiscoveryOptions, DiscoveryProvider, SocketPhase, invalid,
    members,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use maestro_models::{Model, WebSocket, WebSocketRequest};
use std::io;

/// Release a one-shot connection on return, failure or future drop.
struct OwnedSocket(WebSocket);
impl Drop for OwnedSocket {
    fn drop(&mut self) {
        self.0.close();
    }
}
/// Attribute a failure without emitting an additional diagnostic.
fn failure(kind: DiscoveryErrorKind, message: String) -> DiscoveryError {
    DiscoveryError {
        provider: DiscoveryProvider::Lmstudio,
        kind,
        message,
    }
}
/// Retain the native socket cause, text and operation boundary.
fn socket_failure(phase: SocketPhase, cause: io::Error) -> DiscoveryError {
    let message = cause.to_string();
    failure(
        DiscoveryErrorKind::Socket {
            phase,
            cause: Box::new(cause),
        },
        message,
    )
}
/// Report authentication refusal or parsing while retaining native causes.
fn authentication_error(cause: io::Error, prefix: &str) -> DiscoveryError {
    let message = format!("{prefix}{cause}");
    failure(DiscoveryErrorKind::Authentication(Box::new(cause)), message)
}
/// Select the SDK's normalized hostname and non-default port.
fn destination(base: &str) -> Result<String, DiscoveryError> {
    let url = url::Url::parse(base)
        .map_err(|error| failure(DiscoveryErrorKind::InvalidUrl, error.to_string()))?;
    let host = url.host_str().unwrap_or("");
    Ok(format!("ws://{host}:{}/system", url.port().unwrap_or(1234)))
}
/// Receive one complete message without HTTP-style BOM removal.
async fn receive(socket: &mut WebSocket, phase: SocketPhase) -> Result<Vec<u8>, DiscoveryError> {
    socket
        .receive()
        .await
        .map_err(|cause| socket_failure(phase, cause))?
        .ok_or_else(|| {
            socket_failure(
                phase,
                io::Error::new(io::ErrorKind::UnexpectedEof, "WebSocket connection closed"),
            )
        })
}
/// Authenticate before admitting the listing call.
async fn authenticate(socket: &mut WebSocket) -> Result<(), DiscoveryError> {
    let request = serde_json::json!({
        "authVersion": 1,
        "clientIdentifier": STANDARD.encode(rand::random::<[u8;18]>()),
        "clientPasskey": STANDARD.encode(rand::random::<[u8;18]>()),
    });
    socket
        .send(request.to_string().into_bytes())
        .await
        .map_err(|cause| socket_failure(SocketPhase::Authentication, cause))?;
    let bytes = receive(socket, SocketPhase::Authentication).await?;
    wire::authentication(&String::from_utf8_lossy(&bytes))
        .map_err(|error| {
            authentication_error(
                io::Error::other(error),
                "Failed to parse authentication result: ",
            )
        })?
        .map_err(|error| authentication_error(io::Error::other(error), "Failed to authenticate: "))
}
/// Correlate validated envelopes, then admit the entire downloaded-model array.
async fn listing(socket: &mut WebSocket, base: &str) -> Result<Vec<Model>, DiscoveryError> {
    socket
        .send(br#"{"type":"rpcCall","endpoint":"listDownloadedModels","callId":0}"#.to_vec())
        .await
        .map_err(|cause| socket_failure(SocketPhase::Rpc, cause))?;
    loop {
        let bytes = receive(socket, SocketPhase::Rpc).await?;
        let envelope = wire::envelope(&String::from_utf8_lossy(&bytes))
            .map_err(|error| invalid(DiscoveryProvider::Lmstudio, error))?;
        match envelope {
            wire::Envelope::Result { id: 0.0, result } => {
                let records = schema::listing(result.as_ref().map_or("null", |raw| raw.get()))
                    .map_err(|error| invalid(DiscoveryProvider::Lmstudio, error))?;
                return Ok(records
                    .into_iter()
                    .map(|record| record.model(base))
                    .collect());
            }
            wire::Envelope::Error { id: 0.0, error } => {
                let object = members(error.get())
                    .map_err(|error| invalid(DiscoveryProvider::Lmstudio, error))?;
                let message = super::field::<String>(&object, "title")
                    .unwrap_or_else(|_| "Unknown error".into());
                return Err(failure(DiscoveryErrorKind::Rpc(error), message));
            }
            _ => {}
        }
    }
}
/// Discover downloaded LM Studio language models over authenticated system RPC.
///
/// The API key is not used by this protocol. Each invocation owns its connection.
///
/// # Errors
/// Returns URL, socket-phase, authentication, remote RPC or admission failures.
pub async fn discover_lm_studio_models(
    base_url: &str,
    _api_key: Option<&str>,
    options: &DiscoveryOptions,
) -> Result<Vec<Model>, DiscoveryError> {
    let url = destination(base_url)?;
    let socket = (options.connect_web_socket)(WebSocketRequest {
        url,
        headers: indexmap::IndexMap::new(),
    })
    .await
    .map_err(|cause| socket_failure(SocketPhase::Connection, cause))?;
    let mut socket = OwnedSocket(socket);
    authenticate(&mut socket.0).await?;
    listing(&mut socket.0, base_url).await
}

#[cfg(test)]
mod tests;
