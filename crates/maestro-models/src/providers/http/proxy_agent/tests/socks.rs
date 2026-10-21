//! Authentication observations and exact wire frames owned by the proxy tests.

use crate::tests::socks as protocol;
use std::io::{Read, Write};
use std::net::TcpStream;

/// Complete negotiation observed by authentication assertions.
#[derive(Debug)]
pub(super) struct Negotiation {
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
pub(super) fn negotiate(
    socket: &mut TcpStream,
    authenticate: bool,
) -> std::io::Result<Negotiation> {
    negotiate_reply(socket, authenticate, protocol::DEFAULT_REPLY)
}

/// Capture authentication through the shared parser without duplicating the protocol exchange.
pub(super) fn negotiate_reply(
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

/// Finish a rejected exchange through EOF or reset without discarding received bytes.
///
/// # Errors
/// Returns socket failures other than peer reset or broken pipe.
///
/// # Panics
/// Panics when client protocol bytes differ from the recorded exchange.
pub fn failed_frames(socket: &mut TcpStream, expected: &[Frame]) -> std::io::Result<Vec<u8>> {
    match frames(socket, expected) {
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
            ) => {}
        result => result?,
    }
    socket.shutdown(std::net::Shutdown::Write).ok();
    let mut after = Vec::new();
    match socket.read_to_end(&mut after) {
        Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {}
        result => {
            result?;
        }
    }
    Ok(after)
}
