//! A stored entry keeps its spelling when only its separators differ on Windows.
use super::spelled_as;

#[test]
fn a_windows_entry_spelled_with_either_separator_is_already_recorded() {
    assert!(spelled_as("../project/pkg", r"..\project\pkg", true));
    assert!(spelled_as(r"..\project\pkg", r"..\project\pkg", true));
    assert!(!spelled_as("../other/pkg", r"..\project\pkg", true));
}

#[test]
fn a_posix_entry_keeps_a_backslash_as_a_file_character() {
    assert!(spelled_as("../project/pkg", "../project/pkg", false));
    assert!(!spelled_as(r"..\project\pkg", "../project/pkg", false));
}
