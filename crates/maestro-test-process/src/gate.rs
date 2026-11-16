//! FIFO gates a test holds open to control when a native child continues.
//!
//! A child that waits on a gate, such as the `hold` scenario of the `command` family, blocks
//! until the test releases it, so the test observes the child's progress through completion
//! signals instead of sleeps or polling.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

/// A FIFO created and held open for reading and writing by the test.
///
/// Because the test holds both ends, a child opening the FIFO never blocks on its open, and
/// bytes the test writes wait in the FIFO until a child reads them. Dropping the gate removes
/// the FIFO and closes the test's ends: a child already waiting reads the end of the stream
/// and a child that starts later finds no FIFO, so a failed test never leaves a child blocked.
///
/// # Examples
///
/// ```no_run
/// # fn main() -> std::io::Result<()> {
/// use maestro_test_process::gate::Gate;
/// let directory = maestro_test_directory::Scratch::new()?;
/// let permit = Gate::new(&directory.path().join("permit"))?;
/// // Hand `permit.path()` to a child that waits on it, then let one waiting child continue.
/// permit.release()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Gate {
    /// Where the FIFO exists, removed when the gate is dropped.
    path: PathBuf,
    /// The test's own read and write end, which keeps the FIFO open for the children.
    fifo: File,
}

impl Gate {
    /// Create the FIFO at `path` and hold it open.
    ///
    /// # Errors
    ///
    /// Returns the failure to run `mkfifo`, create the FIFO (including an existing `path`),
    /// or open it.
    pub fn new(path: &Path) -> io::Result<Self> {
        // The system executable stays reachable when a test replaces PATH with its fixtures.
        let output = Command::new("/usr/bin/mkfifo")
            .args(["-m", "600", "--"])
            .arg(path)
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "cannot create gate at {}: {}",
                path.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        let fifo = OpenOptions::new().read(true).write(true).open(path)?;
        Ok(Self {
            path: path.to_owned(),
            fifo,
        })
    }

    /// Where a child opens the gate.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Let one waiting child continue by writing it one byte.
    ///
    /// The byte waits in the FIFO when no child reads yet.
    ///
    /// # Errors
    ///
    /// Returns the failure to write the byte.
    pub fn release(&self) -> io::Result<()> {
        (&self.fifo).write_all(&[1])
    }

    /// Wait for a child to write `count` bytes into the gate and return them.
    ///
    /// Blocks until `count` bytes arrive, so it never returns while no child writes; the
    /// gate's own ends stay open, which keeps the read from seeing the end of the stream.
    ///
    /// # Errors
    ///
    /// Returns the failure to read the bytes.
    pub fn take(&self, count: usize) -> io::Result<Vec<u8>> {
        let mut bytes = vec![0; count];
        (&self.fifo).read_exact(&mut bytes)?;
        Ok(bytes)
    }
}

impl Drop for Gate {
    /// Remove the FIFO; the test's open ends close with the gate.
    fn drop(&mut self) {
        // A FIFO already gone leaves nothing to release, so the failure carries no information.
        let _ = std::fs::remove_file(&self.path);
    }
}
