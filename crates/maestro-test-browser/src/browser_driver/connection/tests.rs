//! Terminal browser connections reject commands without waiting for Chromium shutdown.

use super::*;
use crate::browser_driver::{page::raw, tests::block_on};
use futures_util::SinkExt;
use serde_json::json;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

/// A successful close response ends admission before Chromium closes the socket.
#[test]
fn browser_close_rejects_later_tab_creation() {
    block_on(rejects_later_tab("close"));
}

/// A peer's orderly socket close leaves command admission permanently closed.
#[test]
fn peer_eof_rejects_later_tab_creation() {
    block_on(rejects_later_tab("eof"));
}

/// A lost browser process leaves command admission permanently closed.
#[test]
fn peer_loss_rejects_later_tab_creation() {
    block_on(rejects_later_tab("loss"));
}

/// Drive a real connection through each terminal outcome against a controlled peer.
async fn rejects_later_tab(outcome: &'static str) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let peer = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
        if outcome == "close" {
            let call = socket.next().await.unwrap().unwrap();
            let call: Value = serde_json::from_str(call.to_text().unwrap()).unwrap();
            assert_eq!(call["method"], "Browser.close");
            socket
                .send(Message::text(
                    json!({"id":call["id"],"result":{}}).to_string(),
                ))
                .await
                .unwrap();
            // Keep Chromium's socket open until the client retires its connection.
            let ended = socket.next().await;
            assert!(!matches!(ended, Some(Ok(Message::Text(_)))));
        } else if outcome == "eof" {
            socket.close(None).await.unwrap();
        }
    });
    let mut dispatcher = Dispatcher::connect(&format!("ws://{address}"))
        .await
        .unwrap();
    if outcome == "close" {
        raw(&mut dispatcher, None, "Browser.close", json!({}))
            .await
            .unwrap();
    } else {
        assert!(dispatcher.next().await.is_err());
    }

    let result = dispatcher.submit(None, "Target.createTarget", json!({"url":"about:blank"}));
    assert!(result.is_err(), "tab creation admitted after {outcome}");
    let error = result.unwrap_err();
    assert!(error.to_string().contains("connection"), "{error}");
    assert!(dispatcher.next().await.is_err());
    peer.await.unwrap();
}
