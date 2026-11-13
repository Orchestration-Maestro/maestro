//! Shared support compiled only for library tests.

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use maestro_http::test_support::runtime;

#[path = "../tests/support/native_peer.rs"]
pub(crate) mod native_peer;
