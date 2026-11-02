//! Real DNS, outbound sockets, listeners and pending consumer release.
#![cfg(test)]
use super::wasi_support::{COMPONENT, Harness, run};
use futures_util::future::{Either, select};
use serde_json::json;
use std::{cell::Cell, path::Path, rc::Rc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Notify,
};

#[test]
fn component_network_resolves_and_connects() {
    run(async {
        for generation in ["p2", "p3"] {
            outbound(generation).await;
        }
    });
}

/// The peer sends only after the guest's real connection and pending read are observed.
async fn outbound(generation: &str) {
    let host = Harness::load(Path::new(&std::env::var(COMPONENT).unwrap())).await;
    let pending = Rc::new(Notify::new());
    let observed = pending.clone();
    let _subscription = host.bus.controller.on(
        "pending",
        Rc::new(move |value, _| {
            assert_eq!(value["pending"], true);
            observed.notify_one();
            Ok(None)
        }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let invocation = std::pin::pin!(host.request(
        json!({"op":"connect","generation":generation,"port":listener.local_addr().unwrap().port()})
    ));
    let accept = std::pin::pin!(async {
        let (mut peer, _) = listener.accept().await.unwrap();
        let mut ready = [0; 11];
        peer.read_exact(&mut ready).await.unwrap();
        assert_eq!(&ready, b"guest-ready", "{generation}");
        if generation == "p3" {
            pending.notified().await;
        }
        peer
    });
    let Either::Right((mut peer, invocation)) = select(invocation, accept).await else {
        panic!("{generation}: no native connection");
    };
    peer.write_all(b"native-reply\0\xff").await.unwrap();
    peer.shutdown().await.unwrap();
    let observed = invocation.await;
    assert_eq!(observed["resolved"], true, "{generation} name lookup");
    assert_eq!(
        observed["bytes"],
        json!(b"native-reply\0\xff"),
        "{generation} bytes"
    );
}

#[test]
fn component_network_listens_and_accepts() {
    run(async {
        for generation in ["p2", "p3"] {
            inbound(generation).await;
        }
    });
}

/// The guest publishes its actual ephemeral port through an existing callback route.
async fn inbound(generation: &str) {
    let host = Harness::load(Path::new(&std::env::var(COMPONENT).unwrap())).await;
    let port = Rc::new(Cell::new(0_u16));
    let ready = Rc::new(Notify::new());
    let (observed, notice) = (port.clone(), ready.clone());
    let _subscription = host.bus.controller.on(
        "listening",
        Rc::new(move |value, _| {
            observed.set(u16::try_from(value.as_u64().unwrap()).unwrap());
            notice.notify_one();
            Ok(None)
        }),
    );
    let invocation = std::pin::pin!(host.request(json!({"op":"listen","generation":generation})));
    let readiness = std::pin::pin!(ready.notified());
    let Either::Right(((), invocation)) = select(invocation, readiness).await else {
        panic!("{generation}: no native listener");
    };
    assert_ne!(port.get(), 0, "{generation} actual bound port");
    let mut peer = TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port.get()))
        .await
        .unwrap();
    peer.write_all(b"inbound\0\xff\xce\xa9").await.unwrap();
    peer.shutdown().await.unwrap();
    let mut bytes = Vec::new();
    peer.read_to_end(&mut bytes).await.unwrap();
    assert_eq!(bytes, b"guest-reply\0\xff", "{generation} accepted reply");
    let observed = invocation.await;
    assert_eq!(
        observed["bytes"],
        json!(b"inbound\0\xff\xce\xa9"),
        "{generation} accepted input"
    );
}

#[test]
fn component_network_pending_work_releases_once() {
    run(close_pending());
}

/// Both the native completion future and peer EOF witness release of the real retained owner.
async fn close_pending() {
    let path = std::env::var(COMPONENT).unwrap();
    let host = Harness::load(Path::new(&path)).await;
    let independent = Harness::load(Path::new(&path)).await;
    let initial = independent.request(json!({"op":"environment"})).await;
    assert_eq!(initial["calls"], 1);
    let pending = Rc::new(Notify::new());
    let ready = pending.clone();
    let _subscription = host.bus.controller.on(
        "pending",
        Rc::new(move |value, _| {
            assert_pending_ack(&value);
            ready.notify_one();
            Ok(None)
        }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let invocation = std::pin::pin!(
        host.request(json!({"op":"close-pending","port":listener.local_addr().unwrap().port()}))
    );
    let accept = std::pin::pin!(listener.accept());
    let Either::Right((peer, invocation)) = select(invocation, accept).await else {
        panic!("native read did not start");
    };
    let (mut peer, _) = peer.unwrap();
    peer.write_all(b"A").await.unwrap();
    pending.notified().await;
    assert_eq!(
        peer.read(&mut [0]).await.unwrap(),
        0,
        "native peer termination after consumer release"
    );
    let completed = invocation.await;
    assert_closed_result(&completed);
    assert_healthy_stores(&host, &independent, &path).await;
}

/// Pending status follows a byte actually supplied by the native peer.
fn assert_pending_ack(value: &serde_json::Value) {
    assert_eq!(value["pending"], true);
    assert_eq!(value["ack"], 65);
}

/// Consumer release leaves the original and independent Store identities intact.
async fn assert_healthy_stores(host: &Harness, independent: &Harness, path: &str) {
    let healthy = host.request(json!({"op":"environment"})).await;
    assert_eq!(healthy["calls"], 2, "same Store remains usable");
    let healthy = independent.request(json!({"op":"environment"})).await;
    assert_eq!(healthy["calls"], 2, "independent Store remains usable");
    assert_eq!(healthy["p2"][1], path);
    assert_eq!(healthy["p3"][1], json!([path]));
}

/// The native producer completed and the retained owner was destroyed exactly once.
fn assert_closed_result(value: &serde_json::Value) {
    assert_eq!(
        value["released"], 1,
        "one retained socket/stream owner release"
    );
    assert_eq!(value["ack"], 65);
    assert_eq!(value["calls"], 1);
}
