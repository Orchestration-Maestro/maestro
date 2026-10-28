//! Filesystem local sockets with caller-owned names and cleanup identity.

use rustix::io::Errno;
use std::fs;
use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::Path;

/// The owned listener returned by the local-socket library.
pub type LocalListener = interprocess::local_socket::Listener;
/// The owned duplex stream returned by the local-socket library.
pub type LocalStream = interprocess::local_socket::Stream;
/// The library's stream operations, including its native send and receive timeouts.
pub use interprocess::local_socket::traits::Stream as LocalStreamExt;
/// The owned listener of the library's Tokio adapter.
pub type AsyncLocalListener = interprocess::local_socket::tokio::Listener;
/// The owned duplex stream of the library's Tokio adapter.
pub type AsyncLocalStream = interprocess::local_socket::tokio::Stream;
/// The receiving half of an [`AsyncLocalStream`].
pub type AsyncRecvHalf = interprocess::local_socket::tokio::RecvHalf;
/// The sending half of an [`AsyncLocalStream`].
pub type AsyncSendHalf = interprocess::local_socket::tokio::SendHalf;
/// The library's Tokio listener operations, including accepting a connection.
pub use interprocess::local_socket::traits::tokio::Listener as AsyncLocalListenerExt;

/// The observed device and inode of a socket file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SocketFileIdentity {
    /// The device holding the file.
    dev: u64,
    /// The file's inode on that device.
    ino: u64,
}

/// Connect to the supplied filesystem socket path.
///
/// # Errors
/// Returns a path-conversion or connection error.
pub fn connect_local_stream(path: &Path) -> io::Result<LocalStream> {
    use interprocess::local_socket::{GenericFilePath, prelude::*};
    LocalStream::connect(path.to_fs_name::<GenericFilePath>()?)
}

/// Bind without unlinking a name when the listener is dropped.
///
/// # Errors
/// Returns a path-conversion or binding error.
pub fn bind_local_listener(path: &Path) -> io::Result<LocalListener> {
    listener_options(path)?.create_sync()
}

/// Bind for the Tokio adapter without unlinking a name when the listener is dropped. Runs
/// inside a Tokio runtime.
///
/// # Errors
/// Returns a path-conversion or binding error.
pub fn bind_async_local_listener(path: &Path) -> io::Result<AsyncLocalListener> {
    listener_options(path)?.create_tokio()
}

/// The options naming `path` that keep the name when the listener is dropped.
fn listener_options(path: &Path) -> io::Result<interprocess::local_socket::ListenerOptions<'_>> {
    use interprocess::local_socket::{GenericFilePath, ListenerOptions, prelude::*};
    Ok(ListenerOptions::new()
        .name(path.to_fs_name::<GenericFilePath>()?)
        .reclaim_name(false))
}

/// Split a stream into its receiving and sending halves, each owned on its own.
#[must_use]
pub fn split_async_stream(stream: AsyncLocalStream) -> (AsyncRecvHalf, AsyncSendHalf) {
    use interprocess::local_socket::traits::tokio::Stream as _;
    stream.split()
}

/// Create parents and remove a stale socket, rejecting an active listener.
///
/// # Errors
/// Returns address-in-use with caller text, already-exists for a non-socket entry
/// reached after a stale connection failure or a native not-a-socket failure, the
/// native connection failure for a real socket that failed otherwise, or the
/// underlying filesystem/socket error.
pub fn prepare_socket_path(
    path: &Path,
    busy_message: impl FnOnce(&Path) -> String,
) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if !path.exists() && !path.is_symlink() {
        return Ok(());
    }
    let Err(connection_error) = connect_local_stream(path) else {
        return Err(io::Error::new(io::ErrorKind::AddrInUse, busy_message(path)));
    };
    let stale = stale_socket_connect_error(connection_error.kind());
    let not_a_socket = connection_error.raw_os_error() == Some(Errno::NOTSOCK.raw_os_error());
    if !stale && !not_a_socket {
        return Err(connection_error);
    }
    let entry = match fs::symlink_metadata(path) {
        Ok(entry) => entry,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if !entry.file_type().is_socket() {
        return Err(io::ErrorKind::AlreadyExists.into());
    }
    if !stale {
        return Err(connection_error);
    }
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// Recognize stale Unix connection errors without treating back-pressure as stale.
fn stale_socket_connect_error(kind: io::ErrorKind) -> bool {
    matches!(
        kind,
        io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound | io::ErrorKind::TimedOut
    )
}

/// Shut down outgoing bytes while keeping incoming bytes readable.
///
/// # Errors
/// Returns the operating system's shutdown error.
pub fn shutdown_local_stream_write(stream: &LocalStream) -> io::Result<()> {
    match stream {
        LocalStream::UdSocket(stream) => stream.inner().shutdown(std::net::Shutdown::Write),
    }
}

/// Bind an internal listener; the caller then restricts the socket file's permissions.
///
/// # Errors
/// Returns a path-conversion or binding error.
pub fn bind_private_local_listener(path: &Path) -> io::Result<LocalListener> {
    bind_local_listener(path)
}

/// Recognize the terminal connection error kinds.
#[must_use]
pub fn is_connection_closed_error(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::BrokenPipe
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::NotConnected
            | io::ErrorKind::UnexpectedEof
            | io::ErrorKind::WriteZero
    )
}

/// Observe the socket file's identity.
///
/// # Errors
/// Returns the metadata error.
pub fn socket_file_identity(path: &Path) -> io::Result<SocketFileIdentity> {
    let metadata = fs::metadata(path)?;
    Ok(SocketFileIdentity {
        dev: metadata.dev(),
        ino: metadata.ino(),
    })
}

/// Remove the file only when its current identity matches the supplied observation.
///
/// # Errors
/// Returns metadata/removal errors other than a missing file.
pub fn remove_socket_file_if_owned(path: &Path, identity: &SocketFileIdentity) -> io::Result<()> {
    let current = match socket_file_identity(path) {
        Ok(current) => current,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if current != *identity {
        return Ok(());
    }
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// Apply the caller's permission mode to the socket file.
///
/// # Errors
/// Returns metadata or permission-setting errors.
pub fn restrict_socket_permissions(path: &Path, mode: u32) -> io::Result<()> {
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(mode);
    fs::set_permissions(path, permissions)
}

#[cfg(test)]
mod tests;
