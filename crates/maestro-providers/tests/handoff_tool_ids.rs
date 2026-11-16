//! Controlled response identity qualification.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;
#[cfg(not(target_arch = "wasm32"))]
use crate::response_qualification as qualification;

#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
/// Recorded public observations for generated tool ids survive both handoffs.
#[test]
fn maestro_generated_tool_ids_survive_both_handoffs() -> TestResult {
    qualification::assert_rows("maestro_generated_tool_ids_survive_both_handoffs")
}

#[cfg(not(target_arch = "wasm32"))]
/// Recorded public observations for long prefilled ids keep result pairing.
#[test]
fn maestro_long_prefilled_ids_keep_result_pairing() -> TestResult {
    qualification::assert_rows("maestro_long_prefilled_ids_keep_result_pairing")
}

#[cfg(not(target_arch = "wasm32"))]
/// Recorded public observations for handoffs pair each tool occurrence.
#[test]
fn maestro_handoffs_pair_each_tool_occurrence() -> TestResult {
    qualification::assert_rows("maestro_handoffs_pair_each_tool_occurrence")
}
