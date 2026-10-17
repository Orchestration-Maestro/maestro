//! Test identity resolution and the eligibility check.

use std::path::Path;
use std::process::Command;

use crate::TestProcessError;

/// Resolve the running test's full libtest identity from its thread name.
///
/// `test` is the complete identity or its leaf at a `::` boundary.
pub(crate) fn current_name(test: &str) -> Result<String, TestProcessError> {
    let thread = std::thread::current();
    let name = thread
        .name()
        .ok_or_else(|| TestProcessError::new(test, "the current thread is not a named test"))?;
    if name != test && !name.ends_with(&format!("::{test}")) {
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
    if !lists(executable, test, false)? {
        return Err(TestProcessError::new(
            test,
            "the harness lists no test with this exact identity",
        ));
    }
    match (lists(executable, test, true)?, ignored) {
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

/// Ask the harness whether it lists the exact identity, among ignored tests only if asked.
fn lists(executable: &Path, test: &str, ignored: bool) -> Result<bool, TestProcessError> {
    let mut command = Command::new(executable);
    command.args(["--list", "--format", "terse", "--exact", test]);
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
    let wanted = format!("{test}: test");
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|line| line == wanted))
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
        let error = require_eligible(Path::new("/bin/false"), "t", false).unwrap_err();
        assert!(
            error.to_string().contains("the test listing failed"),
            "{error}"
        );
    }
}
