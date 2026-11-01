//! Shared support compiled only for library tests.

#[path = "../tests/support/ftp_server.rs"]
pub(crate) mod ftp_server;
#[path = "../tests/support/loopback.rs"]
pub mod loopback;
#[path = "../tests/support/runtime.rs"]
pub(crate) mod runtime;
#[path = "../tests/support/socks.rs"]
pub mod socks;
#[path = "../tests/support/tls.rs"]
pub mod tls;

#[path = "../tests/support/content_credentials.rs"]
pub mod content_credentials;
#[path = "../tests/support/native_peer.rs"]
pub(crate) mod native_peer;
#[path = "../tests/support/oauth_admission.rs"]
pub(crate) mod oauth_admission;
