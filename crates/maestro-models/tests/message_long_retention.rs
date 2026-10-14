//! Controlled public message qualification.

use crate::chat;
use crate::message_qualification as qualification;

use chat::TestResult;

/// Long-retention probes preserve each selected catalog route and eager compatibility.
#[test]
fn maestro_message_probes_long_retention_per_provider() -> TestResult {
    qualification::assert_rows("maestro_message_probes_long_retention_per_provider")
}

/// Retention markers follow final eligible blocks rather than earlier user turns.
#[test]
fn maestro_message_places_retention_markers() -> TestResult {
    qualification::assert_rows("maestro_message_places_retention_markers")
}

/// Unequal eager and long compatibility switches select independent wire fields.
#[test]
fn maestro_message_keeps_compatibility_policies_independent() -> TestResult {
    qualification::assert_rows("maestro_message_keeps_compatibility_policies_independent")
}
