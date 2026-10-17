//! Completion-based subprocess isolation.
/// Run each global-registry scenario in a completed, exact-test subprocess.
/// # Panics
/// Panics if the child cannot run or fails its assertions.
#[must_use]
pub fn isolated(name: &str) -> bool {
    if std::env::var_os("MAESTRO_DYNAMIC_CHILD").is_some() {
        return false;
    }
    subprocess::run_child(name, &[]);
    true
}

use crate::subprocess;
