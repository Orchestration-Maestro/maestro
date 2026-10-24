//! Shared support compiled only for library tests.

#[path = "../tests/support/ftp_server.rs"]
pub(crate) mod ftp_server;
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_test_process::loopback;
#[path = "../tests/support/runtime.rs"]
pub(crate) mod runtime;
#[path = "../tests/support/socks.rs"]
pub mod socks;
#[path = "../tests/support/tls.rs"]
pub mod tls;

#[path = "../tests/support/content_credentials.rs"]
pub mod content_credentials;
