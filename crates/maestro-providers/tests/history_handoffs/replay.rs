//! Public history qualification witnesses.

#[cfg(not(target_arch = "wasm32"))]
use crate::history_handoffs::{chat::TestResult, qualification};

#[cfg(not(target_arch = "wasm32"))]
/// Recorded public observations for handoffs generate and replay each route.
#[test]
fn maestro_handoffs_generate_and_replay_each_route() -> TestResult {
    qualification::assert_rows("maestro_handoffs_generate_and_replay_each_route")
}

#[cfg(not(target_arch = "wasm32"))]
/// Recorded public observations for handoffs consume other histories in order.
#[test]
fn maestro_handoffs_consume_other_histories_in_order() -> TestResult {
    qualification::assert_rows("maestro_handoffs_consume_other_histories_in_order")
}

#[cfg(not(target_arch = "wasm32"))]
/// Recorded public observations for handoff failures keep payload evidence.
#[test]
fn maestro_handoff_failures_keep_payload_evidence() -> TestResult {
    qualification::assert_rows("maestro_handoff_failures_keep_payload_evidence")
}
