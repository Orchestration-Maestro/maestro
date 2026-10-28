//! Strict recorded recipes and complete public-result comparisons.
#![cfg(test)]
pub use super::resource_output::{output, recorded_output};
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
    let effects = Rc::new(support::Effects::default());
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
    maestro_test_directory::Restore,
    std::path::PathBuf,
)> {
    if maestro_test_process::run_alone(test).unwrap() {
        return None;
    }
    let tree = maestro_test_directory::Scratch::new().unwrap();
    let (process, restore) = tree.enter_unreadable("process").unwrap()?;
    Some((tree, restore, process))
}

/// The variable naming the tree a child builds its case in.
const TREE: &str = "MAESTRO_TEST_TREE";

/// Runs `case` of the calling test in a child process over a fresh tree this process owns.
/// The child starts online, without an inherited `MAESTRO_OFFLINE`, and its temporary
/// directory is `temp` in the tree. `environment` gives the child's other variables for that
/// tree: a value sets one, `None` removes it. Returns the tree in the child selected for
/// `case`; `None` in the caller once the child passed, and in a child selected for another
/// case.
pub fn case_in_child(
    case: &str,
    environment: impl FnOnce(&str) -> Vec<(&'static str, Option<String>)>,
) -> Option<std::path::PathBuf> {
    let selection = maestro_test_process::selection();
    if selection.is_child() {
        return selection
            .runs_case(case)
            .then(|| std::env::var_os(TREE).unwrap().into());
    }
    let tree = super::scratch::Scratch::new().unwrap();
    let base = tree.path().to_str().unwrap();
    let temp = tree.path().join("temp");
    std::fs::create_dir(&temp).unwrap();
    let mut child =
        maestro_test_process::TestProcess::current(std::thread::current().name().unwrap())
            .unwrap()
            .case(case)
            .env(TREE, base)
            .env_remove("MAESTRO_OFFLINE")
            .temp_dir(temp);
    for (name, value) in environment(base) {
        child = match value {
            Some(value) => child.env(name, value),
            None => child.env_remove(name),
        };
    }
    let output = child.output().unwrap();
    assert!(output.status.success(), "{case}: {output:?}");
    None
}

/// Runs `case` of the calling test in a child process, as [`case_in_child`] does, with a
/// working directory that was removed. Returns the tree and the owner of the removed
/// directory in the child selected for `case`; `None` in the caller once the child passed.
#[cfg(unix)]
pub fn case_in_removed_cwd(
    case: &str,
    environment: impl FnOnce(&str) -> Vec<(&'static str, Option<String>)>,
) -> Option<(std::path::PathBuf, super::scratch::Scratch)> {
    let tree = case_in_child(case, environment)?;
    let owner = super::scratch::Scratch::new_in(&tree).unwrap();
    owner.enter_removed("removed").unwrap();
    Some((tree, owner))
}

/// Drives boundary-test futures through the shared test runtime.
///
/// The fixed output type lets each caller's `async` block end in `Ok(())` and use `?`.
pub fn run(operation: impl std::future::Future<Output = io::Result<()>>) -> io::Result<()> {
    crate::runtime::run(operation)
}
