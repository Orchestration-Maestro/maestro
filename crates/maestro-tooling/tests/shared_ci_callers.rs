//! The workflows that call the shared Rust CI.

use std::collections::HashSet;
use std::path::Path;

/// The shared workflow groups push runs by commit, so two callers on one push queue behind each
/// other unless each names its own key.
#[test]
fn callers_of_shared_ci_name_distinct_concurrency_keys() {
    let workflows = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/workflows");
    let mut keys = HashSet::new();
    let mut callers = 0;
    for entry in std::fs::read_dir(workflows).unwrap() {
        let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
        let calls = text
            .lines()
            .filter(|line| {
                line.trim_start().starts_with(
                    "uses: Orchestration-Maestro/maestro-rust-workflows/.github/workflows/ci.yml@",
                )
            })
            .count();
        let file_keys: Vec<&str> = text
            .lines()
            .filter_map(|line| line.trim_start().strip_prefix("concurrency-key:"))
            .map(str::trim)
            .collect();
        assert_eq!(file_keys.len(), calls, "every call names a concurrency key");
        for key in file_keys {
            assert!(!key.is_empty(), "a concurrency key is not empty");
            assert!(keys.insert(key.to_owned()), "{key} is used by two callers");
        }
        callers += calls;
    }
    assert!(callers >= 2, "the full-suite and warm-up callers exist");
}
