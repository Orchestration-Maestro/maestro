//! Controlled reasoning and text qualification.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;

#[cfg(not(target_arch = "wasm32"))]
use crate::reasoning_qualification as qualification;

#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
/// Compare the complete recorded public observations.
#[test]
fn maestro_cloud_catalog_keeps_configured_order() -> TestResult {
    qualification::assert_rows("maestro_cloud_catalog_keeps_configured_order")
}
