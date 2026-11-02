//! The workflows that call the shared Rust CI.

use std::path::Path;

/// Merge groups search the coverage manifest that the default concurrency key names, and the
/// full suite after landing publishes it, so no caller of the shared workflow names a key.
#[test]
fn callers_of_shared_ci_use_the_default_concurrency_key() {
    let workflows = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/workflows");
    let mut callers = 0;
    for entry in std::fs::read_dir(workflows).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        let calls = text.lines().filter(|line| {
            line.trim_start().starts_with(
                "uses: Orchestration-Maestro/maestro-rust-workflows/.github/workflows/ci.yml@",
            )
        });
        callers += calls.count();
        assert!(
            !text.contains("concurrency-key"),
            "{} names a concurrency key",
            path.display()
        );
    }
    assert!(callers >= 1, "the full-suite caller exists");
}
