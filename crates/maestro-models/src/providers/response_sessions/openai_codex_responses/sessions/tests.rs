//! Controlled native observations of retired idle owners and synchronous claims.

use super::super::tests::{exclusive, socket_transport::run_native};
use super::*;
use futures_util::SinkExt;

/// A real loopback connection and its peer.
async fn pair() -> (Socket, WebSocketStream<tokio::net::TcpStream>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let server = async {
        let (stream, _) = listener.accept().await.unwrap();
        tokio_tungstenite::accept_async(stream).await.unwrap()
    };
    let (socket, peer) =
        futures_util::future::join(tokio_tungstenite::connect_async(url), server).await;
    (socket.unwrap().0, peer)
}

/// The common identity of these controlled connections.
fn identity() -> Identity {
    Identity::new("ws://controlled", &IndexMap::new())
}

#[test]
fn maestro_response_sessions_closed_state_is_not_busy() {
    let _isolated = exclusive();
    run_native(async {
        let (socket, mut peer) = pair().await;
        let lease = publish("closed-state", identity(), socket);
        let entry = cached_entry("closed-state").unwrap();
        *lock(&entry.state) = State::Closed;
        assert!(matches!(claim("closed-state", &identity()), Claim::Absent));
        assert!(cached_entry("closed-state").is_none());
        drop(lease);
        assert!(matches!(peer.next().await, Some(Ok(Message::Close(_)))));
    });
}

#[test]
fn maestro_response_sessions_retired_idle_poll_cannot_expire_new_owner() {
    let _isolated = exclusive();
    run_native(async {
        let (socket, mut peer) = pair().await;
        publish("retired-poll", identity(), socket).keep();
        let entry = cached_entry("retired-poll").unwrap();
        let owner = Arc::downgrade(&entry);
        tokio::spawn(async move {
            let mut timer = pin!(sleep_until(Instant::now() - Duration::from_secs(1)));
            poll_fn(|context| poll_idle(&owner, timer.as_mut(), context)).await;
        })
        .await
        .unwrap();
        let Claim::Idle(lease) = claim("retired-poll", &identity()) else {
            panic!("a retired poll must leave the new idle owner claimable");
        };
        assert!(lease.reused);
        drop(lease);
        assert!(matches!(peer.next().await, Some(Ok(Message::Close(_)))));
    });
}

#[test]
fn maestro_response_sessions_claim_drains_ready_idle_close() {
    let _isolated = exclusive();
    run_native(async {
        let (socket, mut peer) = pair().await;
        publish("ready-close", identity(), socket).keep();
        let entry = cached_entry("ready-close").unwrap();
        {
            let state = lock(&entry.state);
            let State::Idle(idle) = &*state else {
                panic!("idle publication")
            };
            idle.task.abort();
        }
        peer.send(Message::text("{malformed")).await.unwrap();
        peer.send(Message::Close(None)).await.unwrap();
        // Descriptor readiness closes the producer set without an idle receive consuming it.
        poll_fn(|context| {
            let state = lock(&entry.state);
            let State::Idle(idle) = &*state else {
                panic!("idle publication")
            };
            let MaybeTlsStream::Plain(stream) = idle.socket.get_ref() else {
                panic!("plain loopback")
            };
            stream.poll_read_ready(context)
        })
        .await
        .unwrap();
        assert!(matches!(claim("ready-close", &identity()), Claim::Absent));
        assert!(cached_entry("ready-close").is_none());
    });
}
