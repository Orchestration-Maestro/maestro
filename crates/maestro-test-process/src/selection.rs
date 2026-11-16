//! Test identity resolution and the eligibility check.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

use crate::TestProcessError;

/// Resolve the running test's full libtest identity from its thread name.
///
/// `test` is the complete identity or its leaf at a `::` boundary.
pub(crate) fn current_name(test: &str) -> Result<String, TestProcessError> {
    let thread = std::thread::current();
    let name = thread
        .name()
        .ok_or_else(|| TestProcessError::new(test, "the current thread is not a named test"))?;
    if !matches_name(name, test) {
        return Err(TestProcessError::new(
            test,
            format!("the selection does not match the running test {name}"),
        ));
    }
    Ok(name.to_owned())
}

/// Derive a libtest identity from a test function item by dropping its crate segment.
///
/// # Errors
///
/// Fails when the item has no module-qualified path to derive an identity from.
pub fn test_name<F: Fn()>(_test: F) -> Result<String, TestProcessError> {
    let path = std::any::type_name::<F>();
    path.split_once("::")
        .map(|(_crate, identity)| identity.to_owned())
        .ok_or_else(|| TestProcessError::new(path, "the item has no crate-qualified path"))
}

/// Require the exact identity to be listed by the harness and eligible for the requested
/// mode: ordinary runs reject an ignored test, ignored runs reject an ordinary one.
pub(crate) fn require_eligible(
    executable: &Path,
    test: &str,
    ignored: bool,
) -> Result<(), TestProcessError> {
    let listing = listing(executable, test)?;
    if !listing.tests.contains(test) {
        return Err(TestProcessError::new(
            test,
            "the harness lists no test with this exact identity",
        ));
    }
    match (listing.ignored.contains(test), ignored) {
        (true, false) => Err(TestProcessError::new(
            test,
            "the selected test is ignored and was not explicitly selected",
        )),
        (false, true) => Err(TestProcessError::new(
            test,
            "ignored execution selected a test that is not ignored",
        )),
        _ => Ok(()),
    }
}

/// The identities one executable's harness lists.
struct Listing {
    /// Every identity the harness lists.
    tests: BTreeSet<String>,
    /// The listed identities the harness runs only when ignored runs are asked for.
    ignored: BTreeSet<String>,
}

/// The listing of `executable`, read on the first launch that needs it.
///
/// One launch asks the harness twice before it starts its child, and a test may start
/// hundreds of children of one executable. The harness of a running executable cannot
/// change its listing, so one read answers every later launch from this process.
static LISTINGS: LazyLock<Mutex<BTreeMap<PathBuf, Arc<Listing>>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));

/// The listing of `executable`, read now or reused from the first launch that needed it.
fn listing(executable: &Path, test: &str) -> Result<Arc<Listing>, TestProcessError> {
    if let Some(listing) = cached_listings().get(executable).cloned() {
        return Ok(listing);
    }
    let listing = Arc::new(Listing {
        tests: lists(executable, test, false)?,
        ignored: lists(executable, test, true)?,
    });
    let mut cached = cached_listings();
    let _previous = cached.insert(executable.to_owned(), Arc::clone(&listing));
    Ok(listing)
}

/// The listings this process has read, waiting while another test's launch holds them.
fn cached_listings() -> MutexGuard<'static, BTreeMap<PathBuf, Arc<Listing>>> {
    // A launch that panicked while holding the listings says nothing about the harness.
    LISTINGS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The identities the harness of `executable` lists, among ignored tests only if asked.
fn lists(
    executable: &Path,
    test: &str,
    ignored: bool,
) -> Result<BTreeSet<String>, TestProcessError> {
    let mut command = Command::new(executable);
    command.args(["--list", "--format", "terse"]);
    if ignored {
        command.arg("--ignored");
    }
    let output = command
        .output()
        .map_err(|error| TestProcessError::new(test, "cannot list tests").caused_by(error))?;
    if !output.status.success() {
        return Err(TestProcessError::new(test, "the test listing failed")
            .with_streams(&output.stdout, &output.stderr));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.strip_suffix(": test").map(str::to_owned))
        .collect())
}

/// Whether a full test identity matches a full name or its leaf at a module boundary.
pub(crate) fn matches_name(name: &str, test: &str) -> bool {
    name == test || name.ends_with(&format!("::{test}"))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::require_eligible;

    /// An executable that cannot be started is reported with its native cause.
    #[test]
    fn missing_executable_fails_the_discovery() {
        let error =
            require_eligible(Path::new("/definitely/missing/test-binary"), "t", false).unwrap_err();
        assert!(error.to_string().contains("cannot list tests"), "{error}");
        assert!(std::error::Error::source(&error).is_some());
    }

    /// A listing that exits unsuccessfully is reported, not read as an empty listing.
    #[cfg(unix)]
    #[test]
    fn failing_listing_fails_the_discovery() {
        let error = require_eligible(Path::new("/usr/bin/false"), "t", false).unwrap_err();
        assert!(
            error.to_string().contains("the test listing failed"),
            "{error}"
        );
    }

    /// One executable answers the harness listing once, so every later launch reuses it.
    ///
    /// The witness removes the executable between two launches: a launch that asked the
    /// harness again would fail on the missing program, and the reused answer still stands.
    #[cfg(unix)]
    #[test]
    fn the_harness_is_listed_once_per_executable() {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let alias = scratch.path().join("test-binary");
        std::os::unix::fs::symlink(std::env::current_exe().unwrap(), &alias).unwrap();
        let test = super::test_name(the_harness_is_listed_once_per_executable).unwrap();
        require_eligible(&alias, &test, false).unwrap();
        std::fs::remove_file(&alias).unwrap();
        require_eligible(&alias, &test, false).unwrap();
    }
}
