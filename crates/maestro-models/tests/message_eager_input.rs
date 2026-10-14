//! Controlled public message qualification.

use crate::chat;
use crate::message_qualification as qualification;

use chat::TestResult;

/// Eager compatibility and tool presence select independent wire branches.
#[test]
fn maestro_message_eager_input_follows_compatibility() -> TestResult {
    qualification::assert_rows("maestro_message_eager_input_follows_compatibility")
}

/// Catalog identities and configured selections retain source order.
#[test]
fn maestro_message_expands_catalog_candidates() -> TestResult {
    qualification::assert_rows("maestro_message_expands_catalog_candidates")
}

/// Fixed catalogs distinguish weighted priorities, stable locale ties and exact-false filtering.
#[test]
fn maestro_message_selects_provider_probes() -> TestResult {
    qualification::assert_rows("maestro_message_selects_provider_probes")
}

/// Configured catalog probes preserve routing and compatibility through completion.
#[test]
fn maestro_message_uses_configured_provider_probes() -> TestResult {
    qualification::assert_rows("maestro_message_uses_configured_provider_probes")
}

/// Forced eager requests select supported candidates before enabling the flag.
#[test]
fn maestro_message_forces_only_supported_eager_probes() -> TestResult {
    qualification::assert_rows("maestro_message_forces_only_supported_eager_probes")
}
