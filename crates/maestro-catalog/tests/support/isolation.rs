//! Completion-based subprocess isolation.
/// Run each global-registry scenario in a completed, exact-test subprocess.
/// # Panics
/// Panics if the child cannot run or fails its assertions.
#[must_use]
pub fn isolated(name: &str) -> bool {
    if maestro_test_process::selection().is_child() {
        return false;
    }
    subprocess::run_child(
        maestro_test_process::TestProcess::current(name)
            .unwrap()
            .case("*"),
        &[],
    );
    true
}

use crate::subprocess;
