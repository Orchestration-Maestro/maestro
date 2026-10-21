//! Tool image placement per protocol.

#[cfg(not(target_arch = "wasm32"))]
use crate::tool_images::{cache_overflow_images::assert_rows, chat::TestResult};

#[cfg(not(target_arch = "wasm32"))]
/// Each protocol emits its own fallback text around tool images.
#[test]
fn maestro_tool_image_fallbacks_keep_protocol_text() -> TestResult {
    assert_rows("maestro_tool_image_fallbacks_keep_protocol_text")
}

#[cfg(not(target_arch = "wasm32"))]
/// Interleaved text and repeated images keep the protocol-defined order.
#[test]
fn maestro_tool_images_keep_block_order_and_duplicates() -> TestResult {
    assert_rows("maestro_tool_images_keep_block_order_and_duplicates")
}
