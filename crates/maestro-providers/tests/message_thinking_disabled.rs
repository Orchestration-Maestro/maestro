//! Controlled public message qualification.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;
#[cfg(not(target_arch = "wasm32"))]
use crate::message_qualification as qualification;

#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
/// Simple absent reasoning explicitly disables the budget-based model.
#[test]
fn maestro_message_disables_budget_thinking() -> TestResult {
    qualification::assert_rows("maestro_message_disables_budget_thinking")
}

#[cfg(not(target_arch = "wasm32"))]
/// Raw missing, false and true thinking differ from simple absent reasoning.
#[test]
fn maestro_message_distinguishes_raw_and_simple_thinking() -> TestResult {
    qualification::assert_rows("maestro_message_distinguishes_raw_and_simple_thinking")
}

#[cfg(not(target_arch = "wasm32"))]
/// Simple absent reasoning explicitly disables both qualified adaptive models.
#[test]
fn maestro_message_disables_adaptive_thinking() -> TestResult {
    qualification::assert_rows("maestro_message_disables_adaptive_thinking")
}

#[cfg(not(target_arch = "wasm32"))]
/// A closed text-only stream emits no thinking and preserves whole-word pong counts.
#[test]
fn maestro_message_off_stream_contains_only_text() -> TestResult {
    qualification::assert_rows("maestro_message_off_stream_contains_only_text")
}
