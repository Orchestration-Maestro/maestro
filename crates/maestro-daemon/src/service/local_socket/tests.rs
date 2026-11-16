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
    let root = maestro_test_directory::Scratch::new_socket().unwrap();
    let path = root.path().join("identity.sock");
    let listener = bind_local_listener(&path).unwrap();
    let identity = socket_file_identity(&path).unwrap();
    let metadata = fs::metadata(&path).unwrap();
    assert_eq!(identity.dev, metadata.dev());
    assert_eq!(identity.ino, metadata.ino());
    drop(listener);
    fs::remove_file(path).unwrap();
}

/// The errno a nonblocking connect reports for a full accept queue: a refusal on Apple, where a
/// full queue refuses like a missing listener.
#[cfg(target_vendor = "apple")]
const FULL_QUEUE_ERRNO: rustix::io::Errno = rustix::io::Errno::CONNREFUSED;
/// The error kind the bounded connect returns for a full accept queue.
#[cfg(target_vendor = "apple")]
const FULL_QUEUE_KIND: io::ErrorKind = io::ErrorKind::ConnectionRefused;
/// The errno a nonblocking connect reports for a full accept queue: would-block on Linux.
#[cfg(not(target_vendor = "apple"))]
const FULL_QUEUE_ERRNO: rustix::io::Errno = rustix::io::Errno::AGAIN;
/// The error kind the bounded connect returns for a full accept queue: the retry on would-block
/// ends in a timeout.
#[cfg(not(target_vendor = "apple"))]
const FULL_QUEUE_KIND: io::ErrorKind = io::ErrorKind::TimedOut;

/// Connect nonblocking clients until the listener's accept queue is full, returning them so the
/// queue stays full.
fn fill_accept_queue(listener: &LocalListener, path: &Path) -> Vec<std::os::fd::OwnedFd> {
    use rustix::net::{SocketAddrUnix, connect, listen};
    let LocalListener::UdSocket(native) = listener;
    listen(native, 1).unwrap();
    let address = SocketAddrUnix::new(path).unwrap();
    let mut clients = Vec::new();
    loop {
        let fd = nonblocking_local_socket().unwrap();
        match connect(&fd, &address) {
            Ok(()) => clients.push(fd),
            Err(error) if error == FULL_QUEUE_ERRNO => return clients,
            Err(error) => panic!("unexpected connect failure: {error}"),
        }
    }
}

#[test]
fn saturated_backlog_never_returns_an_unconnected_stream() {
    let root = maestro_test_directory::Scratch::new_socket().unwrap();
    let path = root.path().join("full.sock");
    let listener = bind_local_listener(&path).unwrap();
    let _clients = fill_accept_queue(&listener, &path);
    let start = Instant::now();
    let result = connect_local_stream_until(&path, start + Duration::from_millis(75));
    assert!(start.elapsed() < Duration::from_secs(1));
    assert_eq!(result.unwrap_err().kind(), FULL_QUEUE_KIND);
}

#[test]
fn claimed_server_with_a_full_accept_queue_keeps_its_socket() {
    let root = maestro_test_directory::Scratch::new_socket().unwrap();
    let path = root.path().join("claimed.sock");
    let _claim = prepare_socket_path(&path, |_| panic!("absent")).unwrap();
    let listener = bind_local_listener(&path).unwrap();
    let identity = socket_file_identity(&path).unwrap();
    let _clients = fill_accept_queue(&listener, &path);
    let error = prepare_socket_path(&path, |_| "busy".to_owned()).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::AddrInUse);
    assert_eq!(socket_file_identity(&path).unwrap(), identity);
}

#[test]
fn bounded_connections_are_close_on_exec() {
    let root = maestro_test_directory::Scratch::new_socket().unwrap();
    let path = root.path().join("cloexec.sock");
    let _listener = bind_local_listener(&path).unwrap();
    let stream =
        connect_local_stream_until(&path, Instant::now() + Duration::from_secs(1)).unwrap();
    let LocalStream::UdSocket(native) = stream;
    let flags = rustix::io::fcntl_getfd(&native).unwrap();
    assert!(flags.contains(rustix::io::FdFlags::CLOEXEC), "{flags:?}");
}
