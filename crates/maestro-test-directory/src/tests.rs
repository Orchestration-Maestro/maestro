//! Collision, creation-failure and name-admission observations at the owner.
use super::{Scratch, allocate};
use std::{
    ffi::OsStr,
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

#[test]
fn existing_candidates_survive_exclusive_allocation() {
    for count in [0, 1, 2, 4] {
        let parent = Scratch::new().unwrap();
        for index in 0..count {
            let candidate = parent
                .path()
                .join(format!("maestro-test-{}-{index}", std::process::id()));
            if index % 2 == 0 {
                fs::write(candidate, "original").unwrap();
            } else {
                fs::create_dir(&candidate).unwrap();
                fs::write(candidate.join("sentinel"), "original").unwrap();
            }
        }
        let sequence = AtomicU64::new(0);
        let allocated = allocate(parent.path(), &sequence).unwrap();
        let attempts = sequence.load(Ordering::Relaxed);
        assert!(attempts <= count + 1, "linear candidate work");
        assert!(allocated.path().is_dir());
        assert_eq!(fs::read_dir(allocated.path()).unwrap().count(), 0);
        for index in 0..count {
            let candidate = parent
                .path()
                .join(format!("maestro-test-{}-{index}", std::process::id()));
            let file = if candidate.is_dir() {
                candidate.join("sentinel")
            } else {
                candidate
            };
            assert_eq!(fs::read_to_string(file).unwrap(), "original");
        }
    }
}

/// A host that folds case answers the same question the independent probe does.
#[test]
fn folded_spellings_report_the_host_answer() {
    let scratch = Scratch::new().unwrap();
    let probe = scratch.path().join("independent");
    fs::create_dir(&probe).unwrap();
    fs::write(probe.join("A.readme"), "").unwrap();
    let folded = probe.join("a.readme").exists();
    assert_eq!(
        scratch
            .keeps_distinct(&[OsStr::new("A.readme"), OsStr::new("a.readme")])
            .unwrap(),
        !folded,
        "folded case spellings"
    );
}

/// A host that refuses native name bytes answers the same question the write does.
#[cfg(unix)]
#[test]
fn refused_names_report_the_host_answer() {
    use std::os::unix::ffi::OsStrExt as _;
    let scratch = Scratch::new().unwrap();
    let probe = scratch.path().join("independent");
    fs::create_dir(&probe).unwrap();
    let name = OsStr::from_bytes(b"n\xff");
    let stored = fs::write(probe.join(name), "").is_ok();
    assert_eq!(
        scratch.keeps_distinct(&[name]).unwrap(),
        stored,
        "refused name bytes"
    );
}
