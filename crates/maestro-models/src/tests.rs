//! Shared support compiled only for library tests.

#[path = "../tests/support/child_process.rs"]
pub mod child_process;
#[path = "../tests/support/loopback.rs"]
pub mod loopback;
#[path = "../tests/support/socks.rs"]
pub mod socks;
#[path = "../tests/support/tls.rs"]
pub mod tls;

#[path = "../tests/support/content_credentials.rs"]
pub mod content_credentials;
