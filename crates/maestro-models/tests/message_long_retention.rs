//! Controlled public message qualification.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;
#[cfg(not(target_arch = "wasm32"))]
use crate::message_qualification as qualification;

#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
/// Long-retention probes preserve each selected catalog route and eager compatibility.
#[test]
fn maestro_message_probes_long_retention_per_provider() -> TestResult {
    qualification::assert_rows("maestro_message_probes_long_retention_per_provider")
}

#[cfg(not(target_arch = "wasm32"))]
/// Retention markers follow final eligible blocks rather than earlier user turns.
#[test]
fn maestro_message_places_retention_markers() -> TestResult {
    qualification::assert_rows("maestro_message_places_retention_markers")
}

#[cfg(not(target_arch = "wasm32"))]
/// Unequal eager and long compatibility switches select independent wire fields.
#[test]
fn maestro_message_keeps_compatibility_policies_independent() -> TestResult {
    qualification::assert_rows("maestro_message_keeps_compatibility_policies_independent")
}
