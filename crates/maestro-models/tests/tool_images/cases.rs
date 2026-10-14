//! Two-turn tool image continuations.

use crate::tool_images::{cache_overflow_images::assert_rows, chat::TestResult};

/// An image-only tool result continues the first turn's returned call.
#[test]
fn maestro_image_results_continue_with_returned_calls() -> TestResult {
    assert_rows("maestro_image_results_continue_with_returned_calls")
}

/// A text and image tool result keeps both blocks on the continuation.
#[test]
fn maestro_mixed_results_preserve_text_and_images() -> TestResult {
    assert_rows("maestro_mixed_results_preserve_text_and_images")
}
