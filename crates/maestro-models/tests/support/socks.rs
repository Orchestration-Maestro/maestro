//! Controlled SOCKS5 peer protocol, using the shared connection owner.

use std::io::{Read, Write};
use std::net::TcpStream;

/// Recorded negotiation, before application traffic.
#[derive(Debug)]
pub struct Negotiation {
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

/// Read one counted byte string.
fn counted(socket: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut size = [0];
    socket.read_exact(&mut size)?;
    let mut bytes = vec![0; usize::from(size[0])];
    socket.read_exact(&mut bytes)?;
    Ok(bytes)
}

/// Negotiate a tunnel, leaving HTTP input unread.
///
/// # Errors
/// Returns protocol socket read or write failures.
pub fn negotiate(socket: &mut TcpStream, authenticate: bool) -> std::io::Result<Negotiation> {
    negotiate_reply(socket, authenticate, &[5, 0, 0, 1, 127, 0, 0, 1, 0, 80])
}

/// Negotiate with a chosen bound-address frame.
///
/// # Errors
/// Returns protocol socket read or write failures.
///
/// # Panics
/// Panics when client bytes differ from the expected protocol input.
pub fn negotiate_reply(
    socket: &mut TcpStream,
    authenticate: bool,
    reply: &[u8],
) -> std::io::Result<Negotiation> {
    let mut version = [0];
    socket.read_exact(&mut version)?;
    assert_eq!(version, [5]);
    let methods = counted(socket)?;
    socket.write_all(&[5, if authenticate { 2 } else { 0 }])?;
    let (user, password) = if authenticate {
        socket.read_exact(&mut version)?;
        assert_eq!(version, [1]);
        let user = counted(socket)?;
        let password = counted(socket)?;
        socket.write_all(&[1, 0])?;
        (Some(user), Some(password))
    } else {
        (None, None)
    };
    let mut head = [0; 4];
    socket.read_exact(&mut head)?;
    assert_eq!(&head[..3], &[5, 1, 0]);
    let host = match head[3] {
        1 | 4 => {
            let mut host = vec![0; if head[3] == 1 { 4 } else { 16 }];
            socket.read_exact(&mut host)?;
            host
        }
        3 => counted(socket)?,
        _ => return Err(std::io::Error::other("invalid destination address")),
    };
    let mut port = [0; 2];
    socket.read_exact(&mut port)?;
    socket.write_all(reply)?;
    Ok(Negotiation {
        methods,
        user,
        password,
        kind: head[3],
        host,
        port: u16::from_be_bytes(port),
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
