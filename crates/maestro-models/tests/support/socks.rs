//! Controlled SOCKS peer protocol, using the shared connection owner.

#![cfg(not(target_arch = "wasm32"))]
use std::io::{Read, Write};
use std::net::TcpStream;

/// Destination observed before application traffic.
#[derive(Debug)]
pub struct Negotiation {
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

/// Ordinary successful bound-address reply.
pub const DEFAULT_REPLY: &[u8] = &[5, 0, 0, 1, 127, 0, 0, 1, 0, 80];

/// Negotiate with a chosen bound-address frame and capture caller-owned authentication observations.
///
/// # Errors
/// Returns protocol socket read or write failures.
///
/// # Panics
/// Panics when client bytes differ from the expected protocol input.
pub fn negotiate_observing<T>(
    socket: &mut TcpStream,
    authenticate: bool,
    reply: &[u8],
    observe: impl FnOnce(Vec<u8>, Option<Vec<u8>>, Option<Vec<u8>>) -> T,
) -> std::io::Result<(Negotiation, T)> {
    let mut version = [0];
    socket.read_exact(&mut version)?;
    if version == [4] {
        let negotiation = read4(socket, observe)?;
        socket.write_all(&[0, 90, 0, 80, 127, 0, 0, 1])?;
        return Ok(negotiation);
    }
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
    Ok((
        Negotiation {
            kind: head[3],
            host,
            port: u16::from_be_bytes(port),
        },
        observe(methods, user, password),
    ))
}

/// Read a NUL-terminated protocol field without consuming the next one.
fn terminated(socket: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    loop {
        let mut byte = [0];
        socket.read_exact(&mut byte)?;
        if byte == [0] {
            return Ok(bytes);
        }
        bytes.push(byte[0]);
    }
}

/// Read a SOCKS4 CONNECT after its version byte, leaving the reply unsent.
///
/// # Errors
/// Returns socket read failures.
///
/// # Panics
/// Panics when the command is not CONNECT.
pub fn read4<T>(
    socket: &mut TcpStream,
    observe: impl FnOnce(Vec<u8>, Option<Vec<u8>>, Option<Vec<u8>>) -> T,
) -> std::io::Result<(Negotiation, T)> {
    let mut head = [0; 7];
    socket.read_exact(&mut head)?;
    assert_eq!(head[0], 1);
    let user = terminated(socket)?;
    let remote = head[3..] == [0, 0, 0, 1];
    let host = if remote {
        terminated(socket)?
    } else {
        head[3..].to_vec()
    };
    Ok((
        Negotiation {
            kind: if remote { 3 } else { 1 },
            host,
            port: u16::from_be_bytes([head[1], head[2]]),
        },
        observe(Vec::new(), Some(user), None),
    ))
}
