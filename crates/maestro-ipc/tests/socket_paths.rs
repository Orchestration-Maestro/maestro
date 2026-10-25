//! Client socket selection from supplied paths and override observations.

use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use maestro_ipc::socket_paths::client_socket_path;

#[test]
fn socket_client_path_uses_api_override() {
    assert_eq!(
        client_socket_path(None, Some("/tmp/api.sock"), None, Path::new("unused")),
        Path::new("/tmp/api-client.sock")
    );
}

#[test]
fn socket_client_path_prefers_api_to_client_override() {
    for (api, client, expected) in [
        ("a.sock", "b.sock", "a-client.sock"),
        ("c.sock", "b.sock", "c-client.sock"),
        ("a.sock", "d.sock", "a-client.sock"),
    ] {
        assert_eq!(
            client_socket_path(None, Some(api), Some(client), Path::new("unused")),
            Path::new(expected)
        );
    }
}

#[test]
fn socket_client_path_honors_client_override() {
    for client in ["named/other.sock", ""] {
        assert_eq!(
            client_socket_path(None, None, Some(client), Path::new("unused")),
            Path::new(client)
        );
    }
}

#[test]
fn socket_client_path_uses_supplied_folder() {
    for folder in ["relative/daemon", "/tmp/named"] {
        assert_eq!(
            client_socket_path(None, None, None, Path::new(folder)),
            Path::new(folder).join("daemon-client.sock")
        );
    }
}

#[test]
fn socket_client_path_default_matches_default_api_derivation() {
    let folder = Path::new("relative/daemon");
    assert_eq!(
        client_socket_path(None, None, None, folder),
        client_socket_path(None, Some("relative/daemon/daemon.sock"), None, folder)
    );
}

#[test]
fn socket_client_path_derives_other_extensions() {
    for (api, expected) in [
        ("/tmp/custom-api", "/tmp/custom-api-client.sock"),
        ("a.tar.gz", "a.tar-client.sock"),
        (".sock", ".sock-client.sock"),
        ("/tmp/trailing/", "/tmp/trailing-client.sock"),
    ] {
        assert_eq!(
            client_socket_path(None, Some(api), None, Path::new("unused")),
            Path::new(expected)
        );
    }
}

#[test]
fn socket_client_path_explicit_daemon_wins() {
    for explicit in ["named/client.sock", ""] {
        assert_eq!(
            client_socket_path(
                Some(Path::new(explicit)),
                Some("api.sock"),
                Some("override.sock"),
                Path::new("unused")
            ),
            Path::new(explicit)
        );
    }
}

#[test]
fn socket_client_path_preserves_native_components() {
    for (api, expected) in [
        ("a/../b.sock", "a/../b-client.sock"),
        ("/tmp/☃.sock", "/tmp/☃-client.sock"),
        ("", "maestro-client.sock"),
        (".", "maestro-client.sock"),
        ("/", "maestro-client.sock"),
    ] {
        assert_eq!(
            client_socket_path(None, Some(api), None, Path::new("unused")),
            Path::new(expected)
        );
    }
    let folder = Path::new(std::ffi::OsStr::from_bytes(b"relative/\xff"));
    assert_eq!(
        client_socket_path(Some(folder), None, None, Path::new("unused")),
        folder
    );
    assert_eq!(
        client_socket_path(None, None, None, folder),
        folder.join("daemon-client.sock")
    );
}
