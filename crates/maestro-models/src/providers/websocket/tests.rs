//! Native socket effect qualification.
use super::*;
use futures_util::{SinkExt as _, StreamExt as _};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::handshake::server::{
    Callback, ErrorResponse, Request, Response,
};
use tokio_tungstenite::{accept_hdr_async, tungstenite::Message};

/// Inspect real upgrade headers.
struct Headers;
impl Callback for Headers {
    fn on_request(self, request: &Request, response: Response) -> Result<Response, ErrorResponse> {
        assert_eq!(request.headers()["x-example"], "supplied");
        Ok(response)
    }
}

#[test]
fn default_socket_connects_sends_receives_and_releases() {
    run(async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let peer = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = accept_hdr_async(stream, Headers).await.unwrap();
            assert_eq!(
                socket.next().await.unwrap().unwrap(),
                Message::Text("request".into())
            );
            socket.send(Message::Ping(vec![7].into())).await.unwrap();
            socket.send(Message::Text("reply".into())).await.unwrap();
            while socket
                .next()
                .await
                .is_some_and(|message| message.is_ok_and(|message| !message.is_close()))
            {
            }
        });
        let mut socket = default_web_socket()(WebSocketRequest {
            url: format!("ws://{address}/system"),
            headers: [("x-example".into(), "supplied".into())].into(),
        })
        .await
        .unwrap();
        socket.send(b"request".to_vec()).await.unwrap();
        assert_eq!(socket.receive().await.unwrap(), Some(b"reply".to_vec()));
        socket.close();
        drop(socket);
        peer.await.unwrap();
    });
}

#[test]
fn socket_effect_keeps_native_failure_sources() {
    run(async {
        use std::error::Error as _;
        let result = default_web_socket()(WebSocketRequest {
            url: "not a socket URL".into(),
            headers: IndexMap::default(),
        })
        .await;
        let error = result.err().unwrap();
        assert!(error.source().is_some());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let peer = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
            socket.send(Message::Close(None)).await.unwrap();
        });
        let mut socket = default_web_socket()(WebSocketRequest {
            url: format!("ws://{address}"),
            headers: IndexMap::default(),
        })
        .await
        .unwrap();
        assert_eq!(socket.receive().await.unwrap(), None);
        let error = socket.send(b"after close".to_vec()).await.unwrap_err();
        assert!(error.source().is_some());
        peer.await.unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let peer = tokio::spawn(async move {
            use tokio::io::AsyncWriteExt as _;
            let (stream, _) = listener.accept().await.unwrap();
            let mut peer = tokio_tungstenite::accept_async(stream).await.unwrap();
            peer.get_mut().write_all(&[0x83, 0]).await.unwrap();
        });
        let mut socket = default_web_socket()(WebSocketRequest {
            url: format!("ws://{address}"),
            headers: IndexMap::new(),
        })
        .await
        .unwrap();
        assert!(socket.receive().await.unwrap_err().source().is_some());
        peer.await.unwrap();
    });
}

#[test]
fn connect_future_drop_releases_partial_upgrade() {
    run(async {
        use tokio::io::AsyncReadExt as _;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (ready, entered) = tokio::sync::oneshot::channel();
        let peer = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut byte = [0];
            assert_eq!(stream.read(&mut byte).await.unwrap(), 1);
            ready.send(()).unwrap();
            let mut rest = Vec::new();
            stream.read_to_end(&mut rest).await.unwrap();
            assert!(rest.ends_with(b"\r\n\r\n"));
        });
        let mut connect = default_web_socket()(WebSocketRequest {
            url: format!("ws://{address}"),
            headers: IndexMap::default(),
        });
        let mut entered = std::pin::pin!(entered);
        std::future::poll_fn(|cx| {
            assert!(connect.as_mut().poll(cx).is_pending());
            entered.as_mut().poll(cx)
        })
        .await
        .unwrap();
        drop(connect);
        peer.await.unwrap();
    });
}

/// Drive a native socket witness.
#[cfg(test)]
fn run(future: impl std::future::Future<Output = ()>) {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future);
}
