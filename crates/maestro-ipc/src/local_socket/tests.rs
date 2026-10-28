//! Native identity fields and the stale-error boundary.
#![cfg(test)]

use super::*;

#[test]
fn socket_stale_errors_exclude_would_block() {
    for kind in [
        io::ErrorKind::ConnectionRefused,
        io::ErrorKind::NotFound,
        io::ErrorKind::TimedOut,
    ] {
        assert!(stale_socket_connect_error(kind), "{kind:?}");
    }
    for kind in [
        io::ErrorKind::WouldBlock,
        io::ErrorKind::PermissionDenied,
        io::ErrorKind::InvalidInput,
        io::ErrorKind::Interrupted,
        io::ErrorKind::ConnectionReset,
    ] {
        assert!(!stale_socket_connect_error(kind), "{kind:?}");
    }
    for errno in [rustix::io::Errno::NOTSOCK, rustix::io::Errno::PROTOTYPE] {
        let error = io::Error::from_raw_os_error(errno.raw_os_error());
        assert!(!stale_socket_connect_error(error.kind()), "{error:?}");
    }
}

#[test]
fn socket_identity_records_native_device_and_inode() {
    let path = std::env::temp_dir().join(format!("mipc-identity-{}.sock", std::process::id()));
    let listener = bind_local_listener(&path).unwrap();
    let identity = socket_file_identity(&path).unwrap();
    let metadata = fs::metadata(&path).unwrap();
    assert_eq!(identity.dev, metadata.dev());
    assert_eq!(identity.ino, metadata.ino());
    drop(listener);
    fs::remove_file(path).unwrap();
}

#[test]
fn saturated_backlog_never_returns_an_unconnected_stream() {
    use rustix::net::{SocketAddrUnix, connect, listen};
    use std::time::{Duration, Instant};
    let root = maestro_test_directory::Scratch::new().unwrap();
    let path = root.path().join("full.sock");
    let listener = bind_local_listener(&path).unwrap();
    let LocalListener::UdSocket(ref native) = listener;
    listen(native, 1).unwrap();
    let address = SocketAddrUnix::new(&path).unwrap();
    let mut clients = Vec::new();
    loop {
        let fd = nonblocking_local_socket().unwrap();
        match connect(&fd, &address) {
            Ok(()) => clients.push(fd),
            Err(rustix::io::Errno::AGAIN) => break,
            Err(error) => panic!("unexpected connect failure: {error}"),
        }
    }
    let start = Instant::now();
    let result = connect_local_stream_until(&path, start + Duration::from_millis(75));
    assert!(start.elapsed() < Duration::from_secs(1));
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
}

#[test]
fn bounded_connections_are_close_on_exec() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    let path = root.path().join("cloexec.sock");
    let _listener = bind_local_listener(&path).unwrap();
    let stream =
        connect_local_stream_until(&path, Instant::now() + Duration::from_secs(1)).unwrap();
    let LocalStream::UdSocket(native) = stream;
    let flags = rustix::io::fcntl_getfd(&native).unwrap();
    assert!(flags.contains(rustix::io::FdFlags::CLOEXEC), "{flags:?}");
}
