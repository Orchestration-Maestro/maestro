//! Build identity.
//! Adapted for Maestro: names, module layout and Rust documentation changed.
#![cfg(test)]
#[test]
/// Build identity exposes a nonempty version string.
fn build_version_exposes_nonempty_text() {
    assert!(!maestro_protocol::build_info::version().is_empty());
}
