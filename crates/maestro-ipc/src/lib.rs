//! Native local-socket transport with caller-supplied paths.

#[cfg(unix)]
pub mod socket_paths;

#[cfg(unix)]
pub mod local_socket;
