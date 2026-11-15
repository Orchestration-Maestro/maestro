//! Chat-specific conclusion policy at its actual adapter.
#[test]
fn chat_conclusion_preserves_recorded_outcomes() {
    crate::tests::conclusions::check_conclusions(true, super::conclude);
}
