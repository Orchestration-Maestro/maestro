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
