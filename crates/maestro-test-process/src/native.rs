//! Controlled native child operations.
#[cfg(unix)]
use std::{path::Path, process::Command};
#[cfg(unix)]
/// Copy `source` to `target` as an executable through a child process.
///
/// # Errors
/// Returns failures starting or collecting the child.
///
/// # Panics
/// Panics if the install command fails.
pub fn install_executable(source: &Path, target: &Path) -> std::io::Result<()> {
    let status = Command::new("install")
        .args(["-m", "0700"])
        .arg(source)
        .arg(target)
        .status()?;
    assert!(status.success(), "install failed");
    Ok(())
}
