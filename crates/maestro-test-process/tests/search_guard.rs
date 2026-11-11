//! Stub and guard search programs answer at once and record a run from outside the root.

use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::Command;

use maestro_test_directory::Scratch;
use maestro_test_process::{SearchGuard, install_stub_search_programs};

/// The command path naming `first` and then `second`.
fn joined(first: &Path, second: &Path) -> OsString {
    std::env::join_paths([first, second]).unwrap()
}

/// Runs `program` found on `path` and returns whether it succeeded with no output.
fn answers_silently(program: &str, path: &OsStr) -> bool {
    let output = Command::new(program).env("PATH", path).output().unwrap();
    output.status.success() && output.stdout.is_empty() && output.stderr.is_empty()
}

#[test]
fn stub_search_programs_answer_with_nothing() {
    let root = Scratch::new().unwrap();
    let bin = root.path().join("bin");
    install_stub_search_programs(&bin).unwrap();
    let path = bin.into_os_string();
    assert!(answers_silently("fd", &path));
    assert!(answers_silently("rg", &path));
}

#[test]
fn guard_records_the_search_programs_a_child_ran_from_outside_the_root() {
    let root = Scratch::new().unwrap();
    let host = Scratch::new().unwrap();
    let guard = SearchGuard::new(root.path(), &joined(root.path(), host.path())).unwrap();
    assert!(guard.reached().unwrap().is_empty());
    guard.assert_unreached();
    assert!(answers_silently("rg", guard.path()));
    assert!(answers_silently("fd", guard.path()));
    assert!(answers_silently("rg", guard.path()));
    assert_eq!(guard.reached().unwrap(), ["fd", "rg"]);
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        guard.assert_unreached();
    }));
    assert!(failed.is_err());
}

#[test]
fn guard_keeps_a_path_that_names_nothing_outside_the_root() {
    let root = Scratch::new().unwrap();
    let inside = root.path().join("bin");
    let path = joined(&inside, &root.path().join("tools"));
    let guard = SearchGuard::new(root.path(), &path).unwrap();
    assert_eq!(guard.path(), path);
}
