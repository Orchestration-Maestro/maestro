//! Shared support compiled only for library tests.

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use maestro_http::test_support::{runtime, socks};
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use maestro_test_harness::loopback;

#[path = "../tests/support/native_peer.rs"]
pub(crate) mod native_peer;

#[path = "../tests/support/content_credentials.rs"]
pub(crate) mod content_credentials;

#[path = "../../maestro-models/tests/support/conclusions.rs"]
pub(crate) mod conclusions;
