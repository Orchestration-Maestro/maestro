//! What a pin observation says about the checkout.
use crate::release::{Need, need};
use maestro_extensions_wasm::package_manager::GitPinStatus;

fn status(expected: Option<&str>, head: Option<&str>, changed: &[&str]) -> GitPinStatus {
    GitPinStatus {
        expected_commit: expected.map(str::to_owned),
        head_commit: head.map(str::to_owned),
        changed_paths: changed.iter().map(|path| (*path).to_owned()).collect(),
    }
}

#[test]
fn an_absent_checkout_is_missing_and_a_clean_one_is_ready() {
    assert_eq!(need(&GitPinStatus::default()), Need::Missing);
    assert_eq!(need(&status(Some("a"), Some("a"), &[])), Need::Clean);
}

#[test]
fn modified_untracked_moved_and_unpinned_checkouts_all_count_as_drift() {
    assert_eq!(
        need(&status(Some("a"), Some("a"), &["content"])),
        Need::Drifted
    );
    assert_eq!(need(&status(Some("a"), Some("b"), &[])), Need::Drifted);
    assert_eq!(need(&status(None, Some("b"), &[])), Need::Drifted);
    assert_eq!(need(&status(Some("a"), None, &[])), Need::Drifted);
}
