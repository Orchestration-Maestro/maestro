//! Native byte decoding observations.

#[cfg(not(target_arch = "wasm32"))]
use crate::provider_unicode::{chat::TestResult, qualification};

#[cfg(not(target_arch = "wasm32"))]
/// Compare the complete recorded public observations.
#[test]
fn maestro_provider_decodes_split_utf8_scalars() -> TestResult {
    qualification::assert_rows("maestro_provider_decodes_split_utf8_scalars")
}

#[cfg(not(target_arch = "wasm32"))]
/// Compare the complete recorded public observations.
#[test]
fn maestro_provider_replaces_invalid_utf8_bytes() -> TestResult {
    qualification::assert_rows("maestro_provider_replaces_invalid_utf8_bytes")
}
