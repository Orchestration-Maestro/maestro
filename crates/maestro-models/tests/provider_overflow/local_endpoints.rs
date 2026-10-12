//! Overflow signals of locally served descriptors.

use crate::{cache_overflow_images::assert_rows, chat::TestResult};

/// Local descriptors report explicit overflow errors but not silent truncation.
#[test]
fn maestro_local_descriptors_keep_overflow_signals() -> TestResult {
    assert_rows("maestro_local_descriptors_keep_overflow_signals")
}
