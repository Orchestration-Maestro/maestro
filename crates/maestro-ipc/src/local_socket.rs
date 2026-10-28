//! Filesystem local sockets with caller-owned names and cleanup identity.

use rustix::io::Errno;
use rustix::net::{AddressFamily, SocketAddrUnix, SocketFlags, SocketType, connect, socket_with};
use std::fs;
use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::Path;
use std::time::{Duration, Instant};

/// The owned listener returned by the local-socket library.
pub type LocalListener = interprocess::local_socket::Listener;
/// The owned duplex stream returned by the local-socket library.
pub type LocalStream = interprocess::local_socket::Stream;
/// The library's stream operations, including its native send and receive timeouts.
pub use interprocess::local_socket::traits::Stream as LocalStreamExt;

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

/// Connect a Unix filesystem socket before the deadline, returning a blocking stream.
///
/// An expired deadline still permits one nonblocking attempt, so probes can distinguish
/// an absent endpoint from a full accept queue without extending the caller's budget.
///
/// # Errors
/// Returns path or connection errors, or timed-out while the accept queue stays full.
pub fn connect_local_stream_until(path: &Path, deadline: Instant) -> io::Result<LocalStream> {
    let address = SocketAddrUnix::new(path)?;
    let fd = nonblocking_local_socket()?;
    loop {
        match connect(&fd, &address) {
            Ok(()) | Err(Errno::ISCONN) => break,
            Err(Errno::AGAIN | Errno::INPROGRESS | Errno::ALREADY | Errno::INTR) => {}
            Err(error) => return Err(error.into()),
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::ErrorKind::TimedOut.into());
        }
        std::thread::sleep(remaining.min(Duration::from_millis(1)));
    }
    let stream = std::os::unix::net::UnixStream::from(fd);
    stream.set_nonblocking(false)?;
    Ok(LocalStream::UdSocket(stream.into()))
}

/// Create an unconnected nonblocking Unix stream descriptor.
fn nonblocking_local_socket() -> io::Result<std::os::fd::OwnedFd> {
    Ok(socket_with(
        AddressFamily::UNIX,
        SocketType::STREAM,
        SocketFlags::NONBLOCK,
        None,
    )?)
}

/// Bind without unlinking a name when the listener is dropped.
///
/// # Errors
/// Returns a path-conversion or binding error.
pub fn bind_local_listener(path: &Path) -> io::Result<LocalListener> {
    use interprocess::local_socket::{GenericFilePath, ListenerOptions, prelude::*};
    ListenerOptions::new()
        .name(path.to_fs_name::<GenericFilePath>()?)
        .reclaim_name(false)
        .create_sync()
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
