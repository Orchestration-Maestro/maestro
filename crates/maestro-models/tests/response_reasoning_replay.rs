//! Controlled response identity qualification.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;
#[cfg(not(target_arch = "wasm32"))]
use crate::response_qualification as qualification;

#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
/// Recorded public observations for replay omits aborted reasoning.
#[test]
fn maestro_replay_omits_aborted_reasoning() -> TestResult {
    qualification::assert_rows("maestro_replay_omits_aborted_reasoning")
}

#[cfg(not(target_arch = "wasm32"))]
/// Recorded public observations for handoff omits foreign reasoning pair ids.
#[test]
fn maestro_handoff_omits_foreign_reasoning_pair_ids() -> TestResult {
    qualification::assert_rows("maestro_handoff_omits_foreign_reasoning_pair_ids")
}

#[cfg(not(target_arch = "wasm32"))]
/// Recorded public observations for handoff preserves response identity.
#[test]
fn maestro_handoff_preserves_response_identity() -> TestResult {
    qualification::assert_rows("maestro_handoff_preserves_response_identity")
}

#[cfg(not(target_arch = "wasm32"))]
/// Recorded public observations for replay selects signatures by full identity.
#[test]
fn maestro_replay_selects_signatures_by_full_identity() -> TestResult {
    qualification::assert_rows("maestro_replay_selects_signatures_by_full_identity")
}
