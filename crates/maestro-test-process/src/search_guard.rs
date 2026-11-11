//! Stub and guard `fd` and `rg` programs for a child's command path.
//!
//! A child that starts an interactive mode looks up both search programs at start-up, so the
//! outcome would otherwise depend on the machine running the test. [`install_stub_search_programs`]
//! supplies answering stand-ins, and a [`SearchGuard`] fails the run when the child still ran a
//! program from outside its test's directories.

use std::ffi::{OsStr, OsString};
use std::io;
use std::path::Path;

use maestro_test_directory::Scratch;
use serde_json::json;

use crate::install_command;

/// The search programs a child looks up.
const PROGRAMS: [&str; 2] = ["fd", "rg"];

/// The file name of `program` on this platform.
fn file_name(program: &str) -> String {
    format!("{program}{}", std::env::consts::EXE_SUFFIX)
}

/// Install stub `fd` and `rg` programs in `bin`, which answer at once with no output and a
/// zero status. Put `bin` ahead of the host directories on the child's command path.
///
/// # Errors
///
/// Returns a missing preparation variable, a failed installation or a failed record write.
/// The directory is created when it is absent.
pub fn install_stub_search_programs(bin: &Path) -> io::Result<()> {
    std::fs::create_dir_all(bin)?;
    let record = json!({"scenario": "output", "stdout": [], "stderr": [], "code": 0});
    for program in PROGRAMS {
        install_command(&bin.join(file_name(program)), &record.to_string())?;
    }
    Ok(())
}

/// A guard `fd` and `rg` on a child's command path, ahead of its first directory outside the
/// child's root, so a child that finds neither its own nor its root's search programs runs a
/// guard copy rather than a host one; every run is recorded.
pub struct SearchGuard {
    /// The disposable directory holding the guard programs and their record.
    directory: Scratch,
    /// The child's command path with the guard directory in place.
    path: OsString,
}

impl SearchGuard {
    /// Guards `path`, the command path of a child over `root`. A path naming no directory
    /// outside `root` keeps its directories as they are.
    ///
    /// # Errors
    ///
    /// Returns a failed directory creation, a missing preparation variable, a failed
    /// installation or a command path that cannot be joined.
    pub fn new(root: &Path, path: &OsStr) -> io::Result<Self> {
        let directory = Scratch::new()?;
        let mut directories: Vec<_> = std::env::split_paths(path).collect();
        let outside = directories
            .iter()
            .position(|entry| entry.is_absolute() && !entry.starts_with(root));
        if let Some(outside) = outside {
            let record = directory.path().join("reached");
            for program in PROGRAMS {
                let scenario = json!({
                    "scenario": "output", "stdout": [], "stderr": [], "code": 0,
                    "append": {
                        "path": record.to_string_lossy(),
                        "cwd": false,
                        "bytes": format!("{program}\n").into_bytes(),
                    },
                });
                install_command(
                    &directory.path().join(file_name(program)),
                    &scenario.to_string(),
                )?;
            }
            directories.insert(outside, directory.path().to_owned());
        }
        let path = std::env::join_paths(directories).map_err(io::Error::other)?;
        Ok(Self { directory, path })
    }

    /// The child's command path with the guard directory in place.
    #[must_use]
    pub fn path(&self) -> &OsStr {
        &self.path
    }

    /// The search programs the child ran from the guard, sorted and without repeats.
    ///
    /// # Errors
    ///
    /// Returns a failed read of the record.
    pub fn reached(&self) -> io::Result<Vec<String>> {
        let record = self.directory.path().join("reached");
        if !record.try_exists()? {
            return Ok(Vec::new());
        }
        let mut names: Vec<String> = std::fs::read_to_string(record)?
            .lines()
            .map(str::to_owned)
            .collect();
        names.sort_unstable();
        names.dedup();
        Ok(names)
    }

    /// Fails the test when the child ran a search program from the guard.
    ///
    /// # Panics
    ///
    /// Panics when the child ran a guard program or the record cannot be read.
    pub fn assert_unreached(&self) {
        let reached = self.reached();
        assert!(
            matches!(&reached, Ok(names) if names.is_empty()),
            "the child ran a search program from outside its test's directories ({reached:?}); \
             a test that supplies the command path installs its own search programs",
        );
    }
}
