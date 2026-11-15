//! Controlled public dispatch observations.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat_tool_choices::{chat::TestResult, qualification};

#[cfg(not(target_arch = "wasm32"))]
/// Stable tool indices retain initial identity when provider identifiers change.
#[test]
fn maestro_public_chat_keeps_stable_tool_indices() -> TestResult {
    qualification::assert_rows("maestro_public_chat_keeps_stable_tool_indices")
}

#[cfg(not(target_arch = "wasm32"))]
/// Interleaved text, reasoning and tool fragments retain their independent order.
#[test]
fn maestro_public_chat_keeps_parallel_tool_deltas_independent() -> TestResult {
    qualification::assert_rows("maestro_public_chat_keeps_parallel_tool_deltas_independent")
}

#[cfg(not(target_arch = "wasm32"))]
/// Reasoning selects the first nonempty field in provider priority order.
#[test]
fn maestro_chat_selects_first_nonempty_reasoning_field() -> TestResult {
    qualification::assert_rows("maestro_chat_selects_first_nonempty_reasoning_field")
}
