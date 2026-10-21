//! Exclusive disposable directory ownership for native tests.
#![cfg(not(target_arch = "wasm32"))]

use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

/// Sequence shared by all callers in this process.
static NEXT: AtomicU64 = AtomicU64::new(0);

/// An exclusively created directory whose removal is attempted on drop.
#[derive(Debug)]
pub struct Scratch {
    /// The spelling selected when this owner was created.
    path: PathBuf,
}

impl Scratch {
    /// Create a directory under the native temporary root and retain its canonical path.
    ///
    /// # Errors
    /// Returns a directory creation or canonicalization failure.
    pub fn new() -> io::Result<Self> {
        let mut scratch = Self::new_in(&std::env::temp_dir())?;
        scratch.path = fs::canonicalize(&scratch.path)?;
        Ok(scratch)
    }

    /// Create a directory under an existing parent using native path joining, without canonicalization.
    ///
    /// # Errors
    /// Returns a directory creation failure other than a candidate name collision.
    pub fn new_in(parent: &Path) -> io::Result<Self> {
        allocate(parent, &NEXT)
    }

    /// The owned directory, with the spelling selected at creation.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Create `name` in this directory, make it the process working directory and remove it.
    /// Reading the working directory then fails with `NotFound`, while `..` still leads back
    /// into this directory. Returns the removed directory. The working directory belongs to
    /// the whole process, so only a process running one test may call this.
    ///
    /// # Errors
    /// Returns a directory creation, working-directory change or removal failure.
    #[cfg(unix)]
    pub fn enter_removed(&self, name: &str) -> io::Result<PathBuf> {
        let removed = self.path.join(name);
        fs::create_dir(&removed)?;
        std::env::set_current_dir(&removed)?;
        fs::remove_dir(&removed)?;
        Ok(removed)
    }

    /// Create `name` in this directory and make a directory below it the process working
    /// directory, deeper than the kernel's path limit, then take read permission from
    /// `name`. Reading the working directory then fails with `PermissionDenied` while paths
    /// relative to it keep working. Returns the working directory and the guard that gives
    /// `name` its permissions back; drop the guard before this owner. The working directory
    /// belongs to the whole process, so only a process running one test may call this. A
    /// privileged process still reads the working directory; callers check for that.
    ///
    /// # Errors
    /// Returns a directory creation, working-directory change or permission failure.
    #[cfg(unix)]
    pub fn enter_unreadable(&self, name: &str) -> io::Result<(PathBuf, Unlock)> {
        use std::os::unix::fs::PermissionsExt;
        let locked = self.path.join(name);
        fs::create_dir(&locked)?;
        std::env::set_current_dir(&locked)?;
        let mut process = locked.clone();
        let segment = "d".repeat(250);
        for _ in 0..20 {
            fs::create_dir(&segment)?;
            std::env::set_current_dir(&segment)?;
            process.push(&segment);
        }
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o311))?;
        Ok((process, Unlock(locked)))
    }
}

/// Gives the ancestor of an unreadable working directory its permissions back on drop.
#[cfg(unix)]
#[derive(Debug)]
pub struct Unlock(PathBuf);

#[cfg(unix)]
impl Drop for Unlock {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o755));
    }
}

/// Retry only existing candidate names, retaining every pre-existing entry.
fn allocate(parent: &Path, sequence: &AtomicU64) -> io::Result<Scratch> {
    loop {
        let path = parent.join(format!(
            "maestro-test-{}-{}",
            std::process::id(),
            sequence.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(Scratch { path }),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests;
