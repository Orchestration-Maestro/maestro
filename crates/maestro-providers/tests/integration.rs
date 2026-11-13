//! Shared integration-test executable.
#![cfg(test)]

#[path = "../../maestro-models/tests/support/clock.rs"]
mod clock;
mod device_account_authorization;
#[cfg(not(target_arch = "wasm32"))]
use maestro_http::test_support::runtime;
#[cfg(not(target_arch = "wasm32"))]
use maestro_test_process::loopback;
#[path = "support/native_peer.rs"]
mod native_peer;
#[path = "support/oauth_admission.rs"]
mod oauth_admission;
mod oauth_primitives;
mod oauth_provider_registry;
mod response_account_authorization;
mod subscription_accounts;
