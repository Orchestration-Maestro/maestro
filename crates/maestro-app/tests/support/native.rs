//! Controlled native command helpers.
use std::path::{Path, PathBuf};

use crate::child;
pub use child::{Scratch, probe};

#[cfg(unix)]
/// Run `git` with fixed identity and no user configuration.
pub fn git(dir: &str, args: &[&str]) -> String {
    let output = maestro_test_harness::git_command::command()
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git starts");
    assert!(output.status.success(), "git {args:?} failed");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[cfg(unix)]
/// Install the prepared native fixture as `name` below `directory`, selected by `record`.
pub fn command(directory: &Path, name: &str, record: &serde_json::Value) -> PathBuf {
    std::fs::create_dir_all(directory).expect("the alias directory is creatable");
    let path = directory.join(name);
    maestro_test_harness::install_command(&path, &record.to_string())
        .expect("the native fixture installs");
    path
}

#[cfg(unix)]
/// Install the prepared fixture as `name` with a `clipboard` record in `root`.
pub fn clipboard_alias(root: &Scratch, name: &str, record: &str) -> std::path::PathBuf {
    let artifact = std::env::var_os("MAESTRO_NATIVE_FIXTURE")
        .expect("run just prepare-tests, then run these tests with cargo nextest");
    let alias =
        std::path::PathBuf::from(root.join(&format!("{name}{}", std::env::consts::EXE_SUFFIX)));
    maestro_test_harness::install_executable(std::path::Path::new(&artifact), &alias).unwrap();
    let mut sibling = alias.as_os_str().to_owned();
    sibling.push(".fixture");
    std::fs::write(sibling, format!("clipboard\n{record}")).unwrap();
    alias
}

#[cfg(unix)]
/// Whether this process is refused access that a mode takes away; a privileged run is not.
pub fn denials() -> bool {
    let probe = maestro_test_harness::Scratch::new().unwrap();
    maestro_test_harness::deny(probe.path(), 0o0)
        .unwrap()
        .is_some()
}
