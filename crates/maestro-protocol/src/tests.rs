//! Build identity behavior.

#[test]
fn default_build_identity_is_stable() {
    assert_eq!(crate::build_info::channel(), "stable");
    assert!(!crate::build_info::is_preview());
    assert_eq!(crate::build_info::build_id(), None);
    assert_eq!(crate::build_info::version(), env!("CARGO_PKG_VERSION"));
}
