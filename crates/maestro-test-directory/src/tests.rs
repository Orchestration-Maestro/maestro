//! Collision and creation-failure observations at the allocator.
use super::{Scratch, allocate};
use std::{
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
