//! Client-socket path selection from caller-supplied observations.

use std::io;
use std::path::{Path, PathBuf};

/// Owner read/write mode for a client socket.
const SOCKET_PERMISSION_MODE: u32 = 0o600;

/// Select explicit daemon path, API override, client override, or the supplied folder.
#[must_use]
pub fn client_socket_path(
    explicit_daemon: Option<&Path>,
    api_socket_override: Option<&str>,
    client_socket_override: Option<&str>,
    daemon_folder: &Path,
) -> PathBuf {
    explicit_daemon.map_or_else(
        || {
            client_socket_path_from_overrides(
                api_socket_override,
                client_socket_override,
                daemon_folder,
            )
        },
        Path::to_path_buf,
    )
}

/// Select an API-derived path, a direct client override, or the folder's client socket.
#[must_use]
fn client_socket_path_from_overrides(
    api_socket_override: Option<&str>,
    client_socket_override: Option<&str>,
    daemon_folder: &Path,
) -> PathBuf {
    if let Some(path) = api_socket_override {
        return derive_client_socket_from_api_socket(Path::new(path));
    }
    client_socket_override.map_or_else(|| daemon_folder.join("daemon-client.sock"), PathBuf::from)
}

/// Derive the client socket name from the API socket's file stem.
#[must_use]
fn derive_client_socket_from_api_socket(api_socket_path: &Path) -> PathBuf {
    let stem = api_socket_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("maestro");
    let parent = api_socket_path.parent().unwrap_or_else(|| Path::new(""));
    parent.join(format!("{stem}-client.sock"))
}

/// Prepare a client socket path with its server-busy diagnostic.
///
/// # Errors
/// Returns the shared socket preparation failure.
pub fn prepare_socket_path(path: &Path) -> io::Result<()> {
    crate::local_socket::prepare_socket_path(path, |path| {
        format!(
            "maestro server is already running (socket busy at {})",
            path.display()
        )
    })
}

/// Restrict a client socket to owner read/write permission.
///
/// # Errors
/// Returns the shared permission-setting failure.
pub fn restrict_socket_permissions(path: &Path) -> io::Result<()> {
    crate::local_socket::restrict_socket_permissions(path, SOCKET_PERMISSION_MODE)
}
