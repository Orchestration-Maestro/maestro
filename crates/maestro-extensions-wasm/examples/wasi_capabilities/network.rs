//! Real native asynchronous sockets with explicit readiness observations.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{EventBus, wasi};
use serde_json::{Value, json};
use std::{
    future::{Future, poll_fn},
    task::Poll,
};
use wasi::sockets::types::{
    IpAddress, IpAddressFamily, IpSocketAddress, Ipv4SocketAddress, TcpSocket,
};

/// A native loopback endpoint; the caller supplies only the ephemeral port.
fn address(port: u16) -> IpSocketAddress {
    IpSocketAddress::Ipv4(Ipv4SocketAddress {
        port,
        address: (127, 0, 0, 1),
    })
}

/// Sends exact bytes through a standard stream, then closes only that view.
pub(super) async fn send(socket: &TcpSocket, bytes: Vec<u8>) -> Result<(), String> {
    let (mut writer, reader) = wasi::wit_stream::new::<u8>();
    let result = socket.send(reader);
    let remaining = writer.write_all(bytes).await;
    if !remaining.is_empty() {
        return Err("socket consumer closed early".into());
    }
    drop(writer);
    result.await.map_err(|error| format!("{error:?}"))
}

/// Resolves a real name and connects through the native standard implementation.
pub(super) async fn connect(request: &Value, events: &EventBus) -> Result<Value, String> {
    let addresses = wasi::sockets::ip_name_lookup::resolve_addresses("localhost".into())
        .await
        .map_err(|error| format!("{error:?}"))?;
    let loopback = addresses
        .iter()
        .any(|address| matches!(address, IpAddress::Ipv4((127, 0, 0, 1))));
    if !loopback {
        return Err("localhost has no loopback result".into());
    }
    let port: u16 =
        serde_json::from_value(request["port"].clone()).map_err(|error| error.to_string())?;
    let socket = TcpSocket::create(IpAddressFamily::Ipv4).map_err(|error| format!("{error:?}"))?;
    socket
        .connect(address(port))
        .await
        .map_err(|error| format!("{error:?}"))?;
    send(&socket, b"guest-ready".to_vec()).await?;
    let (body, result) = socket.receive();
    let read = std::pin::pin!(body.collect());
    let mut read = read;
    let pending = poll_fn(|cx| Poll::Ready(read.as_mut().poll(cx).is_pending())).await;
    events.emit("pending", json!({"pending":pending})).await?;
    let bytes = read.await;
    result.await.map_err(|error| format!("{error:?}"))?;
    Ok(json!({"bytes":bytes,"resolved":true}))
}
