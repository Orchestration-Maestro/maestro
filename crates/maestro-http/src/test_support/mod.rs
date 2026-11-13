//! Controlled peers and runtimes for tests of the transport and of its callers.

pub mod ftp_server;
pub mod ftp_server_timing;
pub mod runtime;
pub mod socks;
pub mod tls;

/// Header layering rows, JSON text shared by the transport tests and by providers'.
pub const HEADER_LAYERS: &str = include_str!("../../tests/fixtures/header_layers.json");
