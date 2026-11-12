//! Setup children that must not reuse the prepared bundle their run inherits.

use std::process::Command;

/// The prepared-bundle inputs a setup child reads from the process that starts it.
const PREPARED_BUNDLE_INPUTS: [&str; 3] =
    ["TEST_PREPARE_BASE", "TEST_PREPARE_OUTPUT", "TEST_ARCHIVE"];

/// Clear the prepared-bundle inputs a setup child inherits, so it runs the setup the test
/// supplies rather than the bundle the run exported for another machine.
pub fn without_prepared_bundles(command: &mut Command) {
    for name in PREPARED_BUNDLE_INPUTS {
        command.env_remove(name);
    }
}
