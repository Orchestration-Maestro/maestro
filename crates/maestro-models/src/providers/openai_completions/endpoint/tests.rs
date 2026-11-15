//! Chat-specific conclusion policy at its actual adapter.
#[test]
fn chat_conclusion_preserves_recorded_outcomes() {
    crate::providers::assistant_output::tests::check_conclusions(true, super::conclude);
}
