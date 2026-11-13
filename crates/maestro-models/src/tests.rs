//! Shared support compiled only for library tests.

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_http::test_support::{ftp_server, runtime, socks, tls};
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_test_process::loopback;

#[path = "../tests/support/content_credentials.rs"]
pub mod content_credentials;
#[path = "../tests/support/login_registry.rs"]
pub(crate) mod login_registry;
#[path = "../tests/support/named_account.rs"]
pub(crate) mod named_account;
#[path = "../tests/support/native_peer.rs"]
pub(crate) mod native_peer;
#[path = "../tests/support/oauth_admission.rs"]
pub(crate) mod oauth_admission;
