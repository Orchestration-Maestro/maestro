//! Controlled public message qualification.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;
#[cfg(not(target_arch = "wasm32"))]
use crate::message_qualification as qualification;

#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
/// Eager compatibility and tool presence select independent wire branches.
#[test]
fn message_eager_input_follows_compatibility() -> TestResult {
    qualification::assert_rows("message_eager_input_follows_compatibility")
}

#[cfg(not(target_arch = "wasm32"))]
/// Catalog identities and configured selections retain source order.
#[test]
fn message_expands_catalog_candidates() -> TestResult {
    qualification::assert_rows("message_expands_catalog_candidates")
}

#[cfg(not(target_arch = "wasm32"))]
/// Fixed catalogs distinguish weighted priorities, stable locale ties and exact-false filtering.
#[test]
fn message_selects_provider_probes() -> TestResult {
    qualification::assert_rows("message_selects_provider_probes")
}

#[cfg(not(target_arch = "wasm32"))]
/// Configured catalog probes preserve routing and compatibility through completion.
#[test]
fn message_uses_configured_provider_probes() -> TestResult {
    qualification::assert_rows("message_uses_configured_provider_probes")
}

#[cfg(not(target_arch = "wasm32"))]
/// Forced eager requests select supported candidates before enabling the flag.
#[test]
fn message_forces_only_supported_eager_probes() -> TestResult {
    qualification::assert_rows("message_forces_only_supported_eager_probes")
}
