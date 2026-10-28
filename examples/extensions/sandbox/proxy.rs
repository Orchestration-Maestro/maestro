//! HTTP CONNECT and SOCKS5 admission and forwarding on loopback listeners.
use super::{
    network::NetworkConfig,
    proxy_io::{self, Reader},
};
use maestro_extensions_wasm::wasi;
use std::net::IpAddr;
use wasi::sockets::types::{
    IpAddress, IpAddressFamily, IpSocketAddress, Ipv4SocketAddress, Ipv6SocketAddress, TcpSocket,
};
use wit_bindgen::rt::async_support::StreamWriter;

/// The protocol accepted by one listener.
#[derive(Clone, Copy)]
pub(crate) enum Protocol {
    /// HTTP requests and CONNECT tunnels.
    Http,
    /// SOCKS5 CONNECT requests.
    Socks,
}
/// The response framing required by the parsed request.
#[derive(Clone, Copy)]
enum Reply {
    /// A successful HTTP CONNECT needs its tunnel acknowledgement.
    Connect,
    /// Ordinary HTTP receives the destination's response.
    Forward,
    /// SOCKS5 replies use the binary connection record.
    Socks,
}
/// One route, with its original body left in the peer's reader.
struct Target {
    /// The hostname the common domain predicate checks before resolution.
    host: String,
    /// Destination port.
    port: u16,
    /// Rewritten origin-form header for ordinary HTTP requests.
    prefix: Vec<u8>,
    /// Response framing.
    reply: Reply,
}
/// A protocol-level admission result.
#[derive(Clone, Copy)]
enum Verdict {
    /// The destination connected after domain admission.
    Accepted,
    /// Domain rules refused the request.
    Denied,
    /// An admitted destination could not connect.
    Failed,
    /// The peer did not send a valid supported request.
    Invalid,
}
/// Whether input reached its natural EOF or was intentionally refused.
enum Settlement {
    /// Both forwarding directions reached EOF.
    Forwarded,
    /// The request ended after a protocol-level refusal.
    Refused,
}

/// Serves one accepted peer without handing its bytes to an unfiltered route.
pub(crate) async fn serve(
    socket: TcpSocket,
    protocol: Protocol,
    rules: &NetworkConfig,
) -> Result<(), String> {
    let (stream, received) = socket.receive();
    let (writer, bytes) = wasi::wit_stream::new::<u8>();
    let sent = socket.send(bytes);
    let settlement = exchange(
        Reader {
            stream,
            buffered: Vec::new(),
        },
        writer,
        protocol,
        rules,
    )
    .await?;
    if matches!(settlement, Settlement::Forwarded) {
        received.await.map_err(|error| format!("{error:?}"))?;
    }
    sent.await.map_err(|error| format!("{error:?}"))
}

/// Parses, admits and then connects; denial never reaches DNS or the destination socket.
async fn exchange(
    mut reader: Reader,
    mut writer: StreamWriter<u8>,
    protocol: Protocol,
    rules: &NetworkConfig,
) -> Result<Settlement, String> {
    let parsed = match protocol {
        Protocol::Http => http(&mut reader).await,
        Protocol::Socks => socks(&mut reader, &mut writer).await,
    };
    let Ok(target) = parsed else {
        return proxy_io::write(
            &mut writer,
            response(default_reply(protocol), Verdict::Invalid),
        )
        .await
        .map(|()| Settlement::Refused);
    };
    if !rules.allows(&target.host) {
        return proxy_io::write(&mut writer, response(target.reply, Verdict::Denied))
            .await
            .map(|()| Settlement::Refused);
    }
    let Ok(destination) = connect(&target.host, target.port).await else {
        return proxy_io::write(&mut writer, response(target.reply, Verdict::Failed))
            .await
            .map(|()| Settlement::Refused);
    };
    proxy_io::write(&mut writer, response(target.reply, Verdict::Accepted)).await?;
    proxy_io::tunnel(reader, writer, destination, target.prefix).await?;
    Ok(Settlement::Forwarded)
}

/// Supplies error framing before a complete target has been parsed.
fn default_reply(protocol: Protocol) -> Reply {
    match protocol {
        Protocol::Http => Reply::Forward,
        Protocol::Socks => Reply::Socks,
    }
}

/// Builds one protocol response; successful ordinary HTTP is answered by the destination.
fn response(reply: Reply, verdict: Verdict) -> Vec<u8> {
    if matches!(reply, Reply::Socks) {
        let code = match verdict {
            Verdict::Accepted => 0,
            Verdict::Denied => 2,
            Verdict::Failed => 5,
            Verdict::Invalid => 1,
        };
        return vec![5, code, 0, 1, 0, 0, 0, 0, 0, 0];
    }
    let status = match verdict {
        Verdict::Accepted if matches!(reply, Reply::Forward) => return Vec::new(),
        Verdict::Accepted => return b"HTTP/1.1 200 Connection Established\r\n\r\n".to_vec(),
        Verdict::Denied => "403 Forbidden",
        Verdict::Failed => "502 Bad Gateway",
        Verdict::Invalid => "400 Bad Request",
    };
    format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\n\r\n").into_bytes()
}

/// Validates a CONNECT authority or an absolute HTTP URL without interpreting shell text.
async fn http(reader: &mut Reader) -> Result<Target, String> {
    let header = reader.header().await?;
    let first = header
        .windows(2)
        .position(|bytes| bytes == b"\r\n")
        .ok_or("Invalid HTTP request")?;
    let line = std::str::from_utf8(&header[..first]).map_err(|error| error.to_string())?;
    let parts: Vec<_> = line.split_ascii_whitespace().collect();
    let [method, uri, version] = parts.as_slice() else {
        return Err("Invalid HTTP request line".into());
    };
    if !matches!(*version, "HTTP/1.0" | "HTTP/1.1") {
        return Err("Invalid HTTP version".into());
    }
    if *method == "CONNECT" {
        let (host, port) = authority(uri, None)?;
        return Ok(Target {
            host,
            port,
            prefix: Vec::new(),
            reply: Reply::Connect,
        });
    }
    let url = uri
        .strip_prefix("http://")
        .ok_or("An HTTP proxy request needs an absolute HTTP URL")?;
    let boundary = url.find(['/', '?', '#']).unwrap_or(url.len());
    let (host, port) = authority(&url[..boundary], Some(80))?;
    let path = url[boundary..].split('#').next().unwrap_or_default();
    let path = if path.starts_with('/') {
        path.to_owned()
    } else {
        format!("/{path}")
    };
    let prefix = forward_header(
        method,
        &path,
        version,
        &header[first + 2..],
        &url[..boundary],
    );
    Ok(Target {
        host,
        port,
        prefix,
        reply: Reply::Forward,
    })
}

/// Preserves opaque header bytes while removing hop-by-hop routing overrides.
fn forward_header(method: &str, path: &str, version: &str, headers: &[u8], host: &str) -> Vec<u8> {
    let mut output =
        format!("{method} {path} {version}\r\nHost: {host}\r\nConnection: close\r\n").into_bytes();
    for line in headers.split(|byte| *byte == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            break;
        }
        let name = line.split(|byte| *byte == b':').next().unwrap_or_default();
        if [b"host".as_slice(), b"connection", b"proxy-connection"]
            .iter()
            .any(|excluded| name.eq_ignore_ascii_case(excluded))
        {
            continue;
        }
        output.extend_from_slice(line);
        output.extend_from_slice(b"\r\n");
    }
    output.extend_from_slice(b"\r\n");
    output
}

/// Parses an explicit port and rejects empty or malformed authorities before resolution.
fn authority(value: &str, default: Option<u16>) -> Result<(String, u16), String> {
    let (host, port) = if let Some(bracketed) = value.strip_prefix('[') {
        let (host, suffix) = bracketed.split_once(']').ok_or("Invalid IPv6 authority")?;
        (
            host,
            suffix
                .strip_prefix(':')
                .map(str::parse)
                .transpose()
                .map_err(|_| "Invalid proxy port")?
                .or(default),
        )
    } else if let Some((host, port)) = value.rsplit_once(':') {
        (host, Some(port.parse().map_err(|_| "Invalid proxy port")?))
    } else {
        (value, default)
    };
    let port = port.filter(|port| *port != 0).ok_or("Missing proxy port")?;
    if host.is_empty()
        || host.chars().any(|character| {
            character.is_whitespace()
                || character.is_control()
                || matches!(character, '@' | '/' | '\\')
        })
    {
        return Err("Invalid proxy hostname".into());
    }
    Ok((host.to_owned(), port))
}

/// Accepts only unauthenticated SOCKS5 CONNECT and retains pipelined tunnel data.
async fn socks(reader: &mut Reader, writer: &mut StreamWriter<u8>) -> Result<Target, String> {
    let greeting = reader.take(2).await?;
    if greeting[0] != 5 {
        return Err("Expected SOCKS5".into());
    }
    let methods = reader.take(usize::from(greeting[1])).await?;
    if !methods.contains(&0) {
        proxy_io::write(writer, vec![5, 255]).await?;
        return Err("SOCKS5 authentication method unavailable".into());
    }
    proxy_io::write(writer, vec![5, 0]).await?;
    let request = reader.take(4).await?;
    if request[..3] != [5, 1, 0] {
        return Err("Expected SOCKS5 CONNECT".into());
    }
    let host = match request[3] {
        1 => {
            let bytes = reader.take(4).await?;
            std::net::Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]).to_string()
        }
        3 => {
            let length = usize::from(reader.take(1).await?[0]);
            String::from_utf8(reader.take(length).await?).map_err(|error| error.to_string())?
        }
        4 => {
            let bytes: [u8; 16] = reader
                .take(16)
                .await?
                .try_into()
                .map_err(|_| "Invalid IPv6 address")?;
            std::net::Ipv6Addr::from(bytes).to_string()
        }
        _ => return Err("Unsupported SOCKS5 address type".into()),
    };
    let bytes = reader.take(2).await?;
    let port = u16::from_be_bytes([bytes[0], bytes[1]]);
    if host.is_empty() || port == 0 {
        return Err("Invalid SOCKS5 destination".into());
    }
    Ok(Target {
        host,
        port,
        prefix: Vec::new(),
        reply: Reply::Socks,
    })
}

/// Connects numeric addresses directly; admitted names use the supplied standard resolver.
async fn connect(host: &str, port: u16) -> Result<TcpSocket, String> {
    let addresses = match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(address)) => {
            let [a, b, c, d] = address.octets();
            vec![IpAddress::Ipv4((a, b, c, d))]
        }
        Ok(IpAddr::V6(address)) => {
            let segments = address.segments();
            vec![IpAddress::Ipv6((
                segments[0],
                segments[1],
                segments[2],
                segments[3],
                segments[4],
                segments[5],
                segments[6],
                segments[7],
            ))]
        }
        Err(_) => wasi::sockets::ip_name_lookup::resolve_addresses(host.to_owned())
            .await
            .map_err(|error| format!("{error:?}"))?,
    };
    for address in addresses {
        let (family, address) = match address {
            IpAddress::Ipv4(address) => (
                IpAddressFamily::Ipv4,
                IpSocketAddress::Ipv4(Ipv4SocketAddress { port, address }),
            ),
            IpAddress::Ipv6(address) => (
                IpAddressFamily::Ipv6,
                IpSocketAddress::Ipv6(Ipv6SocketAddress {
                    port,
                    address,
                    flow_info: 0,
                    scope_id: 0,
                }),
            ),
        };
        let socket = TcpSocket::create(family).map_err(|error| format!("{error:?}"))?;
        if socket.connect(address).await.is_ok() {
            return Ok(socket);
        }
    }
    Err("Cannot connect to proxy destination".into())
}
