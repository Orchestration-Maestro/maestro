//! Controlled cache-preference qualification.

#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/bundled.rs"]
mod bundled;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/cache_overflow_images.rs"]
mod cache_overflow_images;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/chat.rs"]
mod chat;
#[path = "support/child_process.rs"]
mod child_process;
#[path = "support/json.rs"]
mod json;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/messages.rs"]
mod messages;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/response_qualification.rs"]
mod response_qualification;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/transport.rs"]
mod transport;

use cache_overflow_images::assert_rows;
use chat::TestResult;

/// Default, explicit and environment retention reach every protocol request.
#[test]
fn maestro_cache_preferences_reach_each_protocol() -> TestResult {
    assert_rows("maestro_cache_preferences_reach_each_protocol")
}

/// Retention none removes cache markers and the affinity key.
#[test]
fn maestro_cache_none_omits_affinity_key() -> TestResult {
    assert_rows("maestro_cache_none_omits_affinity_key")
}

/// Long-retention support and marker format are independent.
#[test]
fn maestro_cache_flags_keep_independent_markers() -> TestResult {
    assert_rows("maestro_cache_flags_keep_independent_markers")
}
