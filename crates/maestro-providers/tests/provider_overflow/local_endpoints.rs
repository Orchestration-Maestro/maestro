//! Overflow signals of locally served descriptors.

#[cfg(not(target_arch = "wasm32"))]
use crate::provider_overflow::{cache_overflow_images::assert_rows, chat::TestResult};

#[cfg(not(target_arch = "wasm32"))]
/// Local descriptors report explicit overflow errors but not silent truncation.
#[test]
fn maestro_local_descriptors_keep_overflow_signals() -> TestResult {
    assert_rows("maestro_local_descriptors_keep_overflow_signals")
}
