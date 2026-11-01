//! Readiness-driven reads from a native peer socket.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

/// Read peer bytes after readiness, treating shutdown resets or aborts as EOF.
pub(crate) async fn read_peer(peer: &mut tokio::net::TcpStream, bytes: &mut [u8]) -> usize {
    loop {
        peer.readable().await.unwrap();
        match peer.try_read(bytes) {
            Ok(count) => return count,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::ConnectionAborted | std::io::ErrorKind::ConnectionReset
                ) =>
            {
                return 0;
            }
            Err(error) => panic!("{error}"),
        }
    }
}
