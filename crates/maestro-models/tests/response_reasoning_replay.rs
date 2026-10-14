//! Controlled response identity qualification.

use crate::chat;
use crate::response_qualification as qualification;

use chat::TestResult;

/// Recorded public observations for replay omits aborted reasoning.
#[test]
fn maestro_replay_omits_aborted_reasoning() -> TestResult {
    qualification::assert_rows("maestro_replay_omits_aborted_reasoning")
}

/// Recorded public observations for handoff omits foreign reasoning pair ids.
#[test]
fn maestro_handoff_omits_foreign_reasoning_pair_ids() -> TestResult {
    qualification::assert_rows("maestro_handoff_omits_foreign_reasoning_pair_ids")
}

/// Recorded public observations for handoff preserves response identity.
#[test]
fn maestro_handoff_preserves_response_identity() -> TestResult {
    qualification::assert_rows("maestro_handoff_preserves_response_identity")
}

/// Recorded public observations for replay selects signatures by full identity.
#[test]
fn maestro_replay_selects_signatures_by_full_identity() -> TestResult {
    qualification::assert_rows("maestro_replay_selects_signatures_by_full_identity")
}
