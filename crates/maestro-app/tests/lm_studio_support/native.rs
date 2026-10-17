//! Real native socket qualification and retained failure phases.
#![cfg(test)]
use super::{case, empty, group, options};
use futures_util::{SinkExt as _, StreamExt as _};
use maestro_app::browser::utils::model_discovery::{
    DiscoveryErrorKind, SocketPhase, discover_lm_studio_models,
};
use maestro_app::{
    WebSocket, WebSocketConnection, WebSocketFuture, WebSocketRequest, default_web_socket,
};
use serde_json::{Value, json};
use std::{future::Future as _, io, sync::Arc};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::handshake::server::{
    Callback, ErrorResponse, Request, Response,
};
use tokio_tungstenite::{accept_hdr_async, tungstenite::Message};

/// Verify the actual upgrade route and authentication-header absence.
struct SystemHeaders;
impl Callback for SystemHeaders {
    fn on_request(self, request: &Request, response: Response) -> Result<Response, ErrorResponse> {
        assert_eq!(request.uri().path(), "/system");
        assert!(request.headers().get("authorization").is_none());
        Ok(response)
    }
}

/// Run authenticated discovery against a real peer, or drop at both pending phases.
pub async fn native(drop_pending: bool) {
    for phase in if drop_pending { vec![0, 1] } else { vec![2] } {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (ready, entered) = tokio::sync::oneshot::channel();
        let mut recorded = case("ordered_fields_and_duplicates");
        let response = recorded.replies.pop().unwrap();
        let peer = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = accept_hdr_async(stream, SystemHeaders).await.unwrap();
            let auth = socket.next().await.unwrap().unwrap().into_text().unwrap();
            let auth: Value = serde_json::from_str(&auth).unwrap();
            assert_eq!(auth["authVersion"], 1);
            if phase != 0 {
                socket
                    .send(Message::Text(json!({"success":true}).to_string().into()))
                    .await
                    .unwrap();
                let rpc = socket.next().await.unwrap().unwrap().into_text().unwrap();
                assert_eq!(
                    serde_json::from_str::<Value>(&rpc).unwrap(),
                    json!({"type":"rpcCall","endpoint":"listDownloadedModels","callId":0})
                );
            }
            ready.send(()).unwrap();
            if phase == 2 {
                socket
                    .send(Message::Text(response.to_string().into()))
                    .await
                    .unwrap();
            }
            assert!(socket.next().await.is_none_or(
                |message| message.is_err() || message.is_ok_and(|message| message.is_close())
            ));
        });
        let (mut options, _) = options(empty());
        options.connect_web_socket = default_web_socket();
        let base = format!("http://{address}/authored/?q=1#f");
        let mut discovery = Box::pin(discover_lm_studio_models(&base, None, &options));
        if phase == 2 {
            let actual = discovery.await.unwrap();
            let mut expected = recorded.expected.models.unwrap();
            for model in &mut expected {
                model.base_url = model.base_url.replace("$PORT", &address.port().to_string());
            }
            assert_eq!(actual, expected);
            entered.await.unwrap();
        } else {
            let mut entered = std::pin::pin!(entered);
            std::future::poll_fn(|cx| {
                assert!(discovery.as_mut().poll(cx).is_pending());
                entered.as_mut().poll(cx)
            })
            .await
            .unwrap();
            drop(discovery);
        }
        peer.await.unwrap();
    }
}
/// Controlled I/O failure at a specific send boundary.
struct Failing {
    /// Selected phase.
    phase: SocketPhase,
    /// Messages sent before failure.
    sends: usize,
}
impl WebSocketConnection for Failing {
    fn send(&mut self, _: Vec<u8>) -> WebSocketFuture<'_, io::Result<()>> {
        self.sends += 1;
        let fail = self.phase == SocketPhase::Authentication || self.sends == 2;
        Box::pin(async move {
            if fail {
                Err(io::Error::from_raw_os_error(111))
            } else {
                Ok(())
            }
        })
    }
    fn receive(&mut self) -> WebSocketFuture<'_, io::Result<Option<Vec<u8>>>> {
        Box::pin(async { Ok(Some(br#"{"success":true}"#.to_vec())) })
    }
    fn close(&mut self) {}
}
/// Keep the original operating-system error and phase through discovery.
pub async fn failure_phase(phase: SocketPhase) {
    let (mut options, _) = options(empty());
    options.connect_web_socket = Arc::new(move |_| {
        Box::pin(async move {
            if phase == SocketPhase::Connection {
                return Err(io::Error::from_raw_os_error(111));
            }
            Ok(Box::new(Failing { phase, sends: 0 }) as WebSocket)
        })
    });
    let error = discover_lm_studio_models("http://host", None, &options)
        .await
        .unwrap_err();
    let source = std::error::Error::source(&error)
        .unwrap()
        .downcast_ref::<io::Error>()
        .unwrap();
    assert_eq!(source.raw_os_error(), Some(111));
    assert!(
        matches!(error.kind, DiscoveryErrorKind::Socket { phase: actual, .. } if actual == phase)
    );
}
/// Exercise recorded real connection refusal and rejected upgrade routes.
pub async fn connection_failures() {
    for mode in group::<Vec<String>>("connectionFailures") {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let peer = if mode == "refused" {
            drop(listener);
            None
        } else {
            assert_eq!(mode, "upgrade");
            Some(tokio::spawn(reject_upgrade(listener)))
        };
        let error = default_web_socket()(WebSocketRequest {
            url: format!("ws://{address}/system"),
            headers: indexmap::IndexMap::new(),
        })
        .await
        .err()
        .unwrap();
        assert!(std::error::Error::source(&error).is_some());
        if let Some(peer) = peer {
            peer.await.unwrap();
        }
    }
    // The original empty-result operation is also a real native success.
    let selected = case("empty_listing");
    assert!(selected.expected.success);
    super::replay("empty_listing").await;
}

/// Consume the upgrade request before refusing it.
async fn reject_upgrade(listener: TcpListener) {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    let (mut stream, _) = listener.accept().await.unwrap();
    let mut byte = [0];
    let mut request = Vec::new();
    while !request.ends_with(b"\r\n\r\n") {
        stream.read_exact(&mut byte).await.unwrap();
        request.push(byte[0]);
    }
    stream
        .write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n")
        .await
        .unwrap();
}
