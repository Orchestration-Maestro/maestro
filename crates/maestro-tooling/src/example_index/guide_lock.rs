//! Derive the standalone guide project's lock using Cargo's offline resolver.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use super::super::workspace::GUIDE;

/// Distinguish concurrent resolutions in the same process.
static NEXT: AtomicU64 = AtomicU64::new(0);

/// Compare or refresh the guide lock after rendering the example manifest.
///
/// The guide is one crate's own qualification fixture and builds on the crate that supplies the
/// guest SDK, so a workspace that holds neither has no guide lock to derive and skips this.
///
/// # Errors
///
/// Returns an error if filesystem access or offline Cargo resolution fails,
/// or if check mode finds a stale guide lock.
pub(super) fn refresh(
    cargo: &Path,
    checkout: &Path,
    guide: Option<&Path>,
    guest: Option<&Path>,
    check: bool,
) -> io::Result<()> {
    let (Some(crate_root), Some(guest)) = (guide, guest) else {
        return Ok(());
    };
    let checkout = checkout.canonicalize()?;
    let guest = guest.canonicalize()?;
    let fixture = crate_root.join(GUIDE);
    let project = Project::new()?;
    let manifest = fs::read_to_string(fixture.join("Cargo.toml.in"))?.replace(
        "\"../maestro/crates/maestro-extensions-wasm\"",
        &serde_json::to_string(&guest)?,
    );
    fs::write(project.0.join("Cargo.toml"), manifest)?;
    fs::create_dir(project.0.join("src"))?;
    fs::write(project.0.join("src/lib.rs"), "")?;
    fs::copy(checkout.join("Cargo.lock"), project.0.join("Cargo.lock"))?;
    let output = Command::new(cargo)
        .current_dir(&checkout)
        .args([
            "metadata",
            "--offline",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(project.0.join("Cargo.toml"))
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "cannot derive the guide lock from the workspace lock:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let after = fs::read(project.0.join("Cargo.lock"))?;
    let path = fixture.join("Cargo.lock");
    if fs::read(&path)? != after {
        if check {
            return Err(io::Error::other(format!(
                "{} is stale; run just example-index",
                path.display()
            )));
        }
        fs::write(path, after)?;
    }
    Ok(())
}

/// An exclusively created resolver project, removed even when resolution fails.
struct Project(PathBuf);

impl Project {
    /// Allocate without reusing another generator's files.
    fn new() -> io::Result<Self> {
        loop {
            let path = std::env::temp_dir().join(format!(
                "maestro-guide-lock-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
    }
}

impl Drop for Project {
    /// Remove only this invocation's disposable resolution files.
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("cannot remove {}: {error}", self.0.display());
        }
    }
}
