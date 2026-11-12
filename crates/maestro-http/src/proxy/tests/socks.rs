//! Authentication observations and exact wire frames owned by the proxy tests.

use crate::tests::{loopback, socks as protocol};
use std::io::{Read, Write};
use std::net::TcpStream;

/// Complete negotiation observed by authentication assertions.
#[derive(Debug)]
pub(crate) struct Negotiation {
    /// Methods offered in order.
    pub methods: Vec<u8>,
    /// Username bytes, if authentication was selected.
    pub user: Option<Vec<u8>>,
    /// Password bytes, if authentication was selected.
    pub password: Option<Vec<u8>>,
    /// Address type.
    pub kind: u8,
    /// Destination bytes.
    pub host: Vec<u8>,
    /// Destination port.
    pub port: u16,
}

/// Negotiate using the ordinary successful bound-address reply.
pub(crate) fn negotiate(
    socket: &mut TcpStream,
    authenticate: bool,
) -> std::io::Result<Negotiation> {
    negotiate_reply(socket, authenticate, protocol::DEFAULT_REPLY)
}

/// Capture authentication through the shared parser without duplicating the protocol exchange.
pub(crate) fn negotiate_reply(
    socket: &mut TcpStream,
    authenticate: bool,
    reply: &[u8],
) -> std::io::Result<Negotiation> {
    let (destination, (methods, user, password)) =
        protocol::negotiate_observing(socket, authenticate, reply, |methods, user, password| {
            (methods, user, password)
        })?;
    Ok(Negotiation {
        methods,
        user,
        password,
        kind: destination.kind,
        host: destination.host,
        port: destination.port,
    })
}

/// One exact protocol input and its reply.
#[derive(Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    /// Bytes expected from the client.
    pub input: Vec<u8>,
    /// Bytes written by the controlled peer.
    pub output: Vec<u8>,
}

/// Exchange exact frames without reading beyond their boundaries.
///
/// # Errors
/// Returns protocol socket read or write failures.
///
/// # Panics
/// Panics when client bytes differ from the expected protocol input.
pub fn frames(socket: &mut TcpStream, frames: &[Frame]) -> std::io::Result<()> {
    for frame in frames {
        let mut bytes = vec![0; frame.input.len()];
        socket.read_exact(&mut bytes)?;
        assert!(bytes == frame.input, "protocol frame bytes");
        for byte in &frame.output {
            socket.write_all(&[*byte])?;
        }
    }
    Ok(())
}

/// Treat a peer that already closed the connection as the end of the exchange.
///
/// # Errors
/// Returns every failure other than a closed connection.
pub(super) fn settle<T>(result: std::io::Result<T>) -> std::io::Result<()> {
    match result {
        Err(error) if !loopback::client_closed(&error) => Err(error),
        _ => Ok(()),
    }
}

/// Finish a rejected exchange through EOF or a closed connection without discarding received
/// bytes.
///
/// # Errors
/// Returns socket failures other than a closed connection.
///
/// # Panics
/// Panics when client protocol bytes differ from the recorded exchange.
pub fn failed_frames(socket: &mut TcpStream, expected: &[Frame]) -> std::io::Result<Vec<u8>> {
    settle(frames(socket, expected))?;
    socket.shutdown(std::net::Shutdown::Write).ok();
    let mut after = Vec::new();
    settle(socket.read_to_end(&mut after))?;
    Ok(after)
}
