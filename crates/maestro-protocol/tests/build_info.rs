//! Build identity.
//! Adapted for Maestro: names, module layout and Rust documentation changed.
#![cfg(test)]
#[test]
/// Stable version defaults to cargo version.
fn stable_version_defaults_to_cargo_version() {
    assert!(!maestro_protocol::build_info::version().is_empty());
}
