//! Native byte decoding observations.

use crate::provider_unicode::{chat::TestResult, qualification};

/// Compare the complete recorded public observations.
#[test]
fn maestro_provider_decodes_split_utf8_scalars() -> TestResult {
    qualification::assert_rows("maestro_provider_decodes_split_utf8_scalars")
}

/// Compare the complete recorded public observations.
#[test]
fn maestro_provider_replaces_invalid_utf8_bytes() -> TestResult {
    qualification::assert_rows("maestro_provider_replaces_invalid_utf8_bytes")
}
