//! Real filesystem sockets through the public native transport.

use super::support::Directory;
use interprocess::local_socket::traits::Listener as _;
use maestro_ipc::local_socket as local;
use std::fs;
use std::io::{self, Read, Write};

#[test]
fn socket_transport_connects_and_preserves_file_ownership() {
    let directory = Directory::new();
    let path = directory.path().join("transport.sock");
    let listener = local::bind_local_listener(&path).unwrap();
    let mut client = local::connect_local_stream(&path).unwrap();
    let mut server = listener.accept().unwrap();
    client.write_all(b"request").unwrap();
    let mut request = [0; 7];
    server.read_exact(&mut request).unwrap();
    assert_eq!(&request, b"request");
    server.write_all(b"response").unwrap();
    let mut response = [0; 8];
    client.read_exact(&mut response).unwrap();
    assert_eq!(&response, b"response");
    drop(listener);
    assert!(path.exists());
}

#[test]
fn socket_prepare_preserves_non_socket_entries() {
    use std::os::unix::fs::{MetadataExt, symlink};
    if maestro_test_process::run_alone("socket_prepare_preserves_non_socket_entries").unwrap() {
        return;
    }
    let directory = Directory::new();
    let file = directory.path().join("file.sock");
    fs::write(&file, b"user content").unwrap();
    let folder = directory.path().join("folder.sock");
    fs::create_dir(&folder).unwrap();
    let file_link = directory.path().join("file-link.sock");
    symlink(&file, &file_link).unwrap();
    let stale = directory.path().join("stale.sock");
    drop(local::bind_local_listener(&stale).unwrap());
    let stale_link = directory.path().join("stale-link.sock");
    symlink(&stale, &stale_link).unwrap();
    let dangling = directory.path().join("dangling.sock");
    let absent = directory.path().join("absent");
    symlink(&absent, &dangling).unwrap();
    for path in [&file, &folder, &file_link, &stale_link, &dangling] {
        let before = fs::symlink_metadata(path).unwrap();
        let native = local::connect_local_stream(path).unwrap_err();
        let error = local::prepare_socket_path(path, |_| panic!("not live")).unwrap_err();
        assert_eq!(
            error.kind(),
            io::ErrorKind::AlreadyExists,
            "{}: {native:?}",
            path.display()
        );
        let after = fs::symlink_metadata(path).unwrap();
        assert_eq!((after.dev(), after.ino()), (before.dev(), before.ino()));
    }
    assert_eq!(fs::read(&file).unwrap(), b"user content");
    for (link, target) in [
        (&file_link, &file),
        (&stale_link, &stale),
        (&dangling, &absent),
    ] {
        assert_eq!(fs::read_link(link).unwrap(), *target);
    }
    let live = directory.path().join("live.sock");
    let _listener = local::bind_local_listener(&live).unwrap();
    let live_link = directory.path().join("live-link.sock");
    symlink(&live, &live_link).unwrap();
    let error = local::prepare_socket_path(&live_link, |_| "live link".to_owned()).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::AddrInUse);
    assert_eq!(error.to_string(), "live link");
    assert_eq!(fs::read_link(&live_link).unwrap(), live);
}

#[test]
fn socket_write_shutdown_preserves_reverse_direction() {
    if maestro_test_process::run_alone("socket_write_shutdown_preserves_reverse_direction").unwrap()
    {
        return;
    }
    let directory = Directory::new();
    let path = directory.path().join("half-close.sock");
    let listener = local::bind_local_listener(&path).unwrap();
    let mut client = local::connect_local_stream(&path).unwrap();
    let mut server = listener.accept().unwrap();
    client.write_all(b"request").unwrap();
    local::shutdown_local_stream_write(&client).unwrap();
    let mut request = Vec::new();
    server.read_to_end(&mut request).unwrap();
    assert_eq!(request, b"request");
    server.write_all(b"reply").unwrap();
    let mut reply = [0; 5];
    client.read_exact(&mut reply).unwrap();
    assert_eq!(&reply, b"reply");
}

#[test]
fn socket_identity_cleanup_preserves_replacement() {
    let directory = Directory::new();
    let path = directory.path().join("identity.sock");
    let original = local::bind_local_listener(&path).unwrap();
    let identity = local::socket_file_identity(&path).unwrap();
    fs::remove_file(&path).unwrap();
    let _replacement = local::bind_local_listener(&path).unwrap();
    let current = local::socket_file_identity(&path).unwrap();
    assert_ne!(identity, current);
    local::remove_socket_file_if_owned(&path, &identity).unwrap();
    assert_eq!(local::socket_file_identity(&path).unwrap(), current);
    local::remove_socket_file_if_owned(&path, &current).unwrap();
    assert!(!path.exists());
    local::remove_socket_file_if_owned(&path, &current).unwrap();
    drop(original);
    let parent_file = directory.path().join("parent-file");
    fs::write(&parent_file, b"retained").unwrap();
    assert!(local::remove_socket_file_if_owned(&parent_file.join("socket"), &current).is_err());
    assert_eq!(fs::read(parent_file).unwrap(), b"retained");
}

#[test]
fn socket_permissions_are_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let directory = Directory::new();
    let path = directory.path().join("mode.sock");
    let _listener = local::bind_local_listener(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
    local::restrict_socket_permissions(&path, 0o600).unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    maestro_ipc::socket_paths::restrict_socket_permissions(&path).unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(local::restrict_socket_permissions(&directory.path().join("missing"), 0o600).is_err());
}

#[test]
fn socket_private_listener_accepts_local_peer() {
    use std::os::unix::fs::PermissionsExt;
    let directory = Directory::new();
    let path = directory.path().join("private.sock");
    local::prepare_socket_path(&path, |_| panic!("absent")).unwrap();
    let listener = local::bind_private_local_listener(&path).unwrap();
    local::restrict_socket_permissions(&path, 0o600).unwrap();
    let mut client = local::connect_local_stream(&path).unwrap();
    let mut server = listener.accept().unwrap();
    client.write_all(b"private").unwrap();
    let mut bytes = [0; 7];
    server.read_exact(&mut bytes).unwrap();
    assert_eq!(&bytes, b"private");
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn socket_closed_error_kinds_match() {
    for kind in [
        io::ErrorKind::BrokenPipe,
        io::ErrorKind::ConnectionAborted,
        io::ErrorKind::ConnectionReset,
        io::ErrorKind::NotConnected,
        io::ErrorKind::UnexpectedEof,
        io::ErrorKind::WriteZero,
    ] {
        assert!(
            local::is_connection_closed_error(&io::Error::from(kind)),
            "{kind:?}"
        );
    }
    for kind in [
        io::ErrorKind::WouldBlock,
        io::ErrorKind::Interrupted,
        io::ErrorKind::TimedOut,
        io::ErrorKind::PermissionDenied,
        io::ErrorKind::Other,
    ] {
        assert!(
            !local::is_connection_closed_error(&io::Error::from(kind)),
            "{kind:?}"
        );
    }
}

#[test]
fn socket_prepare_rejects_live_listener_with_busy_text() {
    use std::cell::Cell;
    let directory = Directory::new();
    let path = directory.path().join("live.sock");
    let _listener = local::bind_local_listener(&path).unwrap();
    let identity = local::socket_file_identity(&path).unwrap();
    let calls = Cell::new(0);
    let error = local::prepare_socket_path(&path, |supplied| {
        calls.set(calls.get() + 1);
        assert_eq!(supplied, path);
        "caller busy message".to_owned()
    })
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::AddrInUse);
    assert_eq!(error.to_string(), "caller busy message");
    assert_eq!(calls.get(), 1);
    let error = maestro_ipc::socket_paths::prepare_socket_path(&path).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::AddrInUse);
    assert_eq!(
        error.to_string(),
        format!(
            "maestro server is already running (socket busy at {})",
            path.display()
        )
    );
    assert_eq!(local::socket_file_identity(&path).unwrap(), identity);
}

#[test]
fn socket_prepare_removes_stale_file() {
    if maestro_test_process::run_alone("socket_prepare_removes_stale_file").unwrap() {
        return;
    }
    let directory = Directory::new();
    let path = directory.path().join("stale.sock");
    drop(local::bind_local_listener(&path).unwrap());
    assert!(path.exists());
    maestro_ipc::socket_paths::prepare_socket_path(&path).unwrap();
    assert!(!path.exists());
    let _rebound = local::bind_local_listener(&path).unwrap();
    assert!(path.exists());
}

#[test]
fn socket_prepare_creates_parents() {
    let directory = Directory::new();
    let path = directory.path().join("nested/deeper/new.sock");
    local::prepare_socket_path(&path, |_| panic!("missing path is not busy")).unwrap();
    assert!(path.parent().unwrap().is_dir());
    let _listener = local::bind_local_listener(&path).unwrap();
    assert!(path.exists());
}

#[test]
fn socket_prepare_preserves_unclassified_failures() {
    use std::os::unix::fs::MetadataExt;
    let directory = Directory::new();
    let file = directory.path().join("parent-file");
    fs::write(&file, b"retained").unwrap();
    let error =
        local::prepare_socket_path(&file.join("child"), |_| panic!("not live")).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read(&file).unwrap(), b"retained");
    let long = directory.path().join("x".repeat(120));
    fs::write(&long, b"also retained").unwrap();
    let before = fs::symlink_metadata(&long).unwrap();
    let native = local::connect_local_stream(&long).unwrap_err();
    assert_eq!(native.kind(), io::ErrorKind::InvalidInput);
    let error = local::prepare_socket_path(&long, |_| panic!("not live")).unwrap_err();
    assert_eq!(error.kind(), native.kind());
    assert_eq!(error.raw_os_error(), native.raw_os_error());
    assert_eq!(error.to_string(), native.to_string());
    let after = fs::symlink_metadata(&long).unwrap();
    assert_eq!((after.dev(), after.ino()), (before.dev(), before.ino()));
    assert_eq!(fs::read(&long).unwrap(), b"also retained");
    let denied = directory.path().join("denied");
    fs::create_dir(&denied).unwrap();
    let before = fs::symlink_metadata(&denied).unwrap();
    let Some(guard) = maestro_test_directory::deny(&denied, 0).unwrap() else {
        return;
    };
    let child = denied.join("new/child");
    let native = fs::create_dir_all(child.parent().unwrap()).unwrap_err();
    let error = local::prepare_socket_path(&child, |_| panic!("not live")).unwrap_err();
    assert_eq!(native.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(error.kind(), native.kind());
    assert_eq!(error.raw_os_error(), native.raw_os_error());
    assert_eq!(error.to_string(), native.to_string());
    drop(guard);
    let after = fs::symlink_metadata(&denied).unwrap();
    assert_eq!((after.dev(), after.ino()), (before.dev(), before.ino()));
    assert!(!denied.join("new").exists());
}

#[test]
fn socket_prepare_retains_datagram_socket_errors() {
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::net::UnixDatagram;
    let directory = Directory::new();
    let path = directory.path().join("datagram.sock");
    let _socket = UnixDatagram::bind(&path).unwrap();
    let before = fs::symlink_metadata(&path).unwrap();
    let native = local::connect_local_stream(&path).unwrap_err();
    let error = local::prepare_socket_path(&path, |_| panic!("not live")).unwrap_err();
    assert_eq!(error.kind(), native.kind());
    assert_eq!(error.raw_os_error(), native.raw_os_error());
    assert_eq!(error.to_string(), native.to_string());
    let after = fs::symlink_metadata(&path).unwrap();
    assert_eq!((after.dev(), after.ino()), (before.dev(), before.ino()));
}

#[test]
fn socket_pending_data_is_not_peer_eof() {
    if maestro_test_process::run_alone("socket_pending_data_is_not_peer_eof").unwrap() {
        return;
    }
    let directory = Directory::new();
    let path = directory.path().join("continued.sock");
    let listener = local::bind_local_listener(&path).unwrap();
    let mut client = local::connect_local_stream(&path).unwrap();
    let mut server = listener.accept().unwrap();
    client.write_all(b"first\n").unwrap();
    let mut first = [0; 6];
    server.read_exact(&mut first).unwrap();
    assert_eq!(&first, b"first\n");
    let command = b"{\"type\":\"get_state\"}\n";
    client.write_all(command).unwrap();
    let mut received = [0; 21];
    server.read_exact(&mut received).unwrap();
    assert_eq!(&received, command);
    local::shutdown_local_stream_write(&client).unwrap();
    assert_eq!(server.read(&mut [0; 1]).unwrap(), 0);
}
