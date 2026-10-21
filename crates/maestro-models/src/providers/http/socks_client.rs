//! SOCKS exchanges outside the native library's admitted domain.

use std::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_socks::TargetAddr;

/// Negotiate only credential pairs with an empty or overlong field.
pub(super) async fn connect(
    mut socket: TcpStream,
    target: TargetAddr<'_>,
    user: &str,
    password: &str,
) -> io::Result<TcpStream> {
    socket.write_all(&[5, 2, 0, 2]).await?;
    let mut answer = [0; 2];
    socket.read_exact(&mut answer).await?;
    match answer {
        [5, 0] => {}
        [5, 2] => authenticate(&mut socket, user, password).await?,
        _ => return Err(io::Error::other("SOCKS authentication method refused")),
    }
    let mut request = vec![5, 1, 0];
    match target {
        TargetAddr::Ip(address) => {
            match address.ip() {
                std::net::IpAddr::V4(ip) => {
                    request.push(1);
                    request.extend(ip.octets());
                }
                std::net::IpAddr::V6(ip) => {
                    request.push(4);
                    request.extend(ip.octets());
                }
            }
            request.extend(address.port().to_be_bytes());
        }
        TargetAddr::Domain(host, port) => {
            request.extend([3, length(host.len())?]);
            request.extend(host.as_bytes());
            request.extend(port.to_be_bytes());
        }
    }
    socket.write_all(&request).await?;
    consume_reply(&mut socket).await?;
    Ok(socket)
}

/// Validate wire lengths only after the proxy chooses password authentication.
async fn authenticate(socket: &mut TcpStream, user: &str, password: &str) -> io::Result<()> {
    let mut frame = vec![1, length(user.len())?];
    frame.extend(user.as_bytes());
    frame.push(length(password.len())?);
    frame.extend(password.as_bytes());
    socket.write_all(&frame).await?;
    let mut answer = [0; 2];
    socket.read_exact(&mut answer).await?;
    if answer != [1, 0] {
        return Err(io::Error::other("SOCKS authentication refused"));
    }
    Ok(())
}

/// Consume exactly the bound-address frame, leaving application bytes in the socket.
async fn consume_reply(socket: &mut TcpStream) -> io::Result<()> {
    let mut head = [0; 4];
    socket.read_exact(&mut head).await?;
    if head[..3] != [5, 0, 0] {
        return Err(io::Error::other("SOCKS connection refused"));
    }
    let size = match head[3] {
        1 => 4,
        4 => 16,
        3 => usize::from(socket.read_u8().await?),
        _ => return Err(io::Error::other("Invalid SOCKS bound address")),
    };
    let mut address = vec![0; size + 2];
    socket.read_exact(&mut address).await?;
    Ok(())
}

/// Require a representable SOCKS counted field, including the empty field.
fn length(size: usize) -> io::Result<u8> {
    u8::try_from(size).map_err(|_| io::Error::other("SOCKS field exceeds 255 bytes"))
}

/// Complete a SOCKS4 exchange beyond the native adapter's admitted inputs.
pub(super) async fn connect4(
    mut socket: TcpStream,
    target: TargetAddr<'_>,
    user: &str,
) -> io::Result<TcpStream> {
    let (port, address, domain) = match target {
        TargetAddr::Ip(std::net::SocketAddr::V4(address)) => {
            (address.port(), address.ip().octets(), None)
        }
        TargetAddr::Ip(std::net::SocketAddr::V6(address)) => (
            address.port(),
            [0, 0, 0, 1],
            Some(std::borrow::Cow::Owned(address.ip().to_string())),
        ),
        TargetAddr::Domain(host, port) => (port, [0, 0, 0, 1], Some(host)),
    };
    let mut request = vec![4, 1];
    request.extend(port.to_be_bytes());
    request.extend(address);
    request.extend(user.as_bytes());
    request.push(0);
    if let Some(domain) = domain {
        request.extend(domain.as_bytes());
        request.push(0);
    }
    socket.write_all(&request).await?;
    let mut reply = [0; 8];
    socket.read_exact(&mut reply).await?;
    if reply[0] != 0 || reply[1] != 90 {
        return Err(io::Error::other("SOCKS connection refused"));
    }
    Ok(socket)
}
