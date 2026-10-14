//! Controlled cache-preference qualification.

use crate::cache_overflow_images;
use crate::chat;

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
