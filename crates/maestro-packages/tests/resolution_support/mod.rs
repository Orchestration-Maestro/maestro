//! Strict recorded recipes and complete public-result comparisons.
#![cfg(test)]
pub use super::resource_output::output;
use super::support;
use maestro_packages::{DefaultPackageManager, PackageManagerOptions};
use serde_json::json;
use std::{io, rc::Rc};
/// Constructs a native-file manager with controlled package effects.
pub fn manager(
    base: &str,
) -> (
    DefaultPackageManager<support::Controlled>,
    Rc<support::Effects>,
) {
    let (_, settings, _) = support::manager(&json!({}));
    let effects = Rc::new(support::Effects {
        native_exists: true,
        ..Default::default()
    });
    (
        DefaultPackageManager::new(
            PackageManagerOptions {
                cwd: base.into(),
                agent_dir: base.into(),
                settings_manager: settings,
            },
            support::Controlled(Rc::clone(&effects)),
        ),
        effects,
    )
}

/// Run the calling test alone with a working directory nobody can read, below a fresh tree.
/// Returns the tree, the guard that restores it and the working directory in that process;
/// `None` in the caller once that run passed, and in a privileged run, which reads it anyway.
#[cfg(unix)]
pub fn unreadable_cwd(
    test: &str,
) -> Option<(
    maestro_test_directory::Scratch,
    maestro_test_directory::Unlock,
    std::path::PathBuf,
)> {
    if maestro_test_process::run_alone(test).unwrap() {
        return None;
    }
    let tree = maestro_test_directory::Scratch::new().unwrap();
    let (process, unlock) = tree.enter_unreadable("process").unwrap();
    if std::env::current_dir().is_ok() {
        drop(unlock);
        return None;
    }
    Some((tree, unlock, process))
}

/// Drives boundary-test futures through the shared test runtime.
///
/// The fixed output type lets each caller's `async` block end in `Ok(())` and use `?`.
pub fn run(operation: impl std::future::Future<Output = io::Result<()>>) -> io::Result<()> {
    crate::runtime::run(operation)
}
