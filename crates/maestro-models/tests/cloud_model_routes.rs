//! Controlled reasoning and text qualification.

use crate::chat;

use crate::reasoning_qualification as qualification;

use chat::TestResult;

/// Compare the complete recorded public observations.
#[test]
fn maestro_cloud_catalog_keeps_configured_order() -> TestResult {
    qualification::assert_rows("maestro_cloud_catalog_keeps_configured_order")
}
