//! Real standard sockets with explicit readiness and retained-consumer observations.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{EventBus, wasi};
use serde_json::{Value, json};
use std::{
    cell::Cell,
    future::{Future, poll_fn},
    io::{Read, Write},
    net::ToSocketAddrs,
    rc::Rc,
    task::Poll,
};
use wasi::sockets::types::{
    IpAddress, IpAddressFamily, IpSocketAddress, Ipv4SocketAddress, TcpSocket,
};

/// A native loopback listener; the OS selects its actual ephemeral port.
fn address(port: u16) -> IpSocketAddress {
    IpSocketAddress::Ipv4(Ipv4SocketAddress {
        port,
        address: (127, 0, 0, 1),
    })
}

/// Sends exact bytes and then closes only this socket's send stream.
async fn send(socket: &TcpSocket, bytes: Vec<u8>) -> Result<(), String> {
    let (mut writer, reader) = wasi::wit_stream::new::<u8>();
    let result = socket.send(reader);
    let remaining = writer.write_all(bytes).await;
    if !remaining.is_empty() {
        return Err("socket consumer closed early".into());
    }
    drop(writer);
    result.await.map_err(|error| format!("{error:?}"))
}

/// Connects to an address actually returned by the standard name resolver.
async fn resolved_socket(request: &Value) -> Result<TcpSocket, String> {
    let addresses = wasi::sockets::ip_name_lookup::resolve_addresses("localhost".into())
        .await
        .map_err(|error| format!("{error:?}"))?;
    let resolved = addresses
        .into_iter()
        .find_map(|address| match address {
            IpAddress::Ipv4(address) => Some(address),
            IpAddress::Ipv6(_) => None,
        })
        .ok_or("localhost has no IPv4 result")?;
    let port: u16 =
        serde_json::from_value(request["port"].clone()).map_err(|error| error.to_string())?;
    let socket = TcpSocket::create(IpAddressFamily::Ipv4).map_err(|error| format!("{error:?}"))?;
    socket
        .connect(IpSocketAddress::Ipv4(Ipv4SocketAddress {
            port,
            address: resolved,
        }))
        .await
        .map_err(|error| format!("{error:?}"))?;
    Ok(socket)
}

/// Resolves a real name and exchanges bytes through either standard implementation.
pub(super) async fn connect(request: &Value, events: &EventBus) -> Result<Value, String> {
    if request["generation"] == "p2" {
        return ordinary_connect(request);
    }
    let socket = resolved_socket(request).await?;
    send(&socket, b"guest-ready".to_vec()).await?;
    let bytes = receive(&socket, events).await?;
    Ok(json!({"bytes":bytes,"resolved":true}))
}

/// Uses Rust's standard-library 0.2 resolver and socket imports, not a literal-only connect.
fn ordinary_connect(request: &Value) -> Result<Value, String> {
    let port: u16 =
        serde_json::from_value(request["port"].clone()).map_err(|error| error.to_string())?;
    let address = ("localhost", port)
        .to_socket_addrs()
        .map_err(|error| error.to_string())?
        .find(std::net::SocketAddr::is_ipv4)
        .ok_or("localhost has no IPv4 result")?;
    let mut socket = std::net::TcpStream::connect(address).map_err(|error| error.to_string())?;
    socket
        .write_all(b"guest-ready")
        .map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    socket
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    Ok(json!({"bytes":bytes,"resolved":true}))
}

/// Distinguishes a genuinely pending native read from an already-completed response.
async fn receive(socket: &TcpSocket, events: &EventBus) -> Result<Vec<u8>, String> {
    let (body, result) = socket.receive();
    let mut read = std::pin::pin!(body.collect());
    let first = poll_fn(|cx| Poll::Ready(read.as_mut().poll(cx))).await;
    events
        .emit("pending", json!({"pending":first.is_pending()}))
        .await?;
    let bytes = match first {
        Poll::Ready(bytes) => bytes,
        Poll::Pending => read.await,
    };
    result.await.map_err(|error| format!("{error:?}"))?;
    Ok(bytes)
}

/// Publishes the bound port over the existing event bus before accepting a native connection.
pub(super) async fn listen(request: &Value, events: &EventBus) -> Result<Value, String> {
    if request["generation"] == "p2" {
        return ordinary_listen(events).await;
    }
    let socket = TcpSocket::create(IpAddressFamily::Ipv4).map_err(|error| format!("{error:?}"))?;
    socket
        .bind(address(0))
        .map_err(|error| format!("{error:?}"))?;
    let mut incoming = socket.listen().map_err(|error| format!("{error:?}"))?;
    let IpSocketAddress::Ipv4(local) = socket
        .get_local_address()
        .map_err(|error| format!("{error:?}"))?
    else {
        return Err("wrong listener address family".into());
    };
    events.emit("listening", json!(local.port)).await?;
    let accepted = incoming
        .next()
        .await
        .ok_or("listener ended before accept")?;
    let (body, completion) = accepted.receive();
    let bytes = body.collect().await;
    completion.await.map_err(|error| format!("{error:?}"))?;
    send(&accepted, b"guest-reply\0\xff".to_vec()).await?;
    Ok(json!({"bytes":bytes}))
}

/// The ordinary standard-library listener shares the user's native network permissions.
async fn ordinary_listen(events: &EventBus) -> Result<Value, String> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
    events
        .emit(
            "listening",
            json!(
                listener
                    .local_addr()
                    .map_err(|error| error.to_string())?
                    .port()
            ),
        )
        .await?;
    let (mut peer, _) = listener.accept().map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    peer.read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    peer.write_all(b"guest-reply\0\xff")
        .map_err(|error| error.to_string())?;
    Ok(json!({"bytes":bytes}))
}

/// Owns the real native socket and its pending standard stream until consumer release.
struct Retained {
    /// The control resource stays alive while the read is pending.
    _socket: TcpSocket,
    /// Dropping this consumer closes the native producer.
    body: wit_bindgen::rt::async_support::StreamReader<u8>,
    /// Counts destruction of this retained owner, not calls into a fake host.
    releases: Rc<Cell<u32>>,
}
impl Drop for Retained {
    fn drop(&mut self) {
        self.releases.set(self.releases.get() + 1);
    }
}

/// Releases an acknowledged, genuinely pending reader without canceling a caller's signal.
pub(super) async fn close_pending(request: &Value, events: &EventBus) -> Result<Value, String> {
    let socket = resolved_socket(request).await?;
    let (body, completion) = socket.receive();
    let releases = Rc::new(Cell::new(0));
    let mut retained = Retained {
        _socket: socket,
        body,
        releases: Rc::clone(&releases),
    };
    let ack = retained
        .body
        .next()
        .await
        .ok_or("peer closed before acknowledgement")?;
    {
        let mut read = std::pin::pin!(retained.body.next());
        let pending = poll_fn(|cx| Poll::Ready(read.as_mut().poll(cx).is_pending())).await;
        if !pending {
            return Err("peer did not leave the read pending".into());
        }
        events
            .emit("pending", json!({"pending":true,"ack":ack}))
            .await?;
    }
    drop(retained);
    completion.await.map_err(|error| format!("{error:?}"))?;
    Ok(json!({"released":releases.get(),"ack":ack}))
}
