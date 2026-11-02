//! Native Git stand-ins for footer observations, served by the prepared fixture artifact.

use std::io;
use std::path::Path;
use std::str::FromStr;

/// Install a stand-in selected by `scenario` from the prepared `MAESTRO_NATIVE_FIXTURE` artifact.
///
/// The caller owns `entry` and its sibling `.fixture` record. The parent directory must exist.
/// Launch the alias where the product launches Git: the stand-in answers the operands, the
/// working directory and the standard descriptors that launch supplies.
///
/// # Examples
///
/// After preparing the native fixture, install an alias in a disposable directory:
///
/// ```no_run
/// use maestro_test_process::git_command::{self, Scenario};
///
/// # fn main() -> std::io::Result<()> {
/// let directory = maestro_test_directory::Scratch::new()?;
/// git_command::install(&directory.path().join("git"), Scenario::Observed)?;
/// # Ok(())
/// # }
/// ```
///
/// # Errors
///
/// Returns an error when preparation is missing, executable installation fails or the record
/// cannot be written.
pub fn install(entry: &Path, scenario: Scenario) -> io::Result<()> {
    crate::executable::install_recorded(entry, "git", scenario.name())
}

/// What a stand-in Git executable observes or produces.
///
/// The observed and reported scenarios answer one lookup and end with the status in
/// `GIT_EXIT`, which defaults to zero. The exiting scenario writes an unterminated prefix and
/// leaves a real descendant holding the same output descriptor; that descendant announces the
/// exiting process's identifier and waits for one release byte. `GIT_LOG` names the observation
/// file and `GIT_HOLDER` names the descendant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    /// Log the operands, directory and descriptors, then answer a branch.
    Observed,
    /// Print the operands and directory the product supplied.
    Reported,
    /// Write a prefix and exit while a real descendant retains the output descriptor.
    Exiting,
    /// Wait for the test's release byte before completing the retained output.
    Retaining,
}

impl Scenario {
    /// The exact selector written into the installed alias's sibling record.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Observed => "Observed",
            Self::Reported => "Reported",
            Self::Exiting => "Exiting",
            Self::Retaining => "Retaining",
        }
    }
}

impl FromStr for Scenario {
    /// A selector outside the compiled scenario set.
    type Err = io::Error;

    /// Reject selectors outside the compiled scenario set.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "Observed" => Ok(Self::Observed),
            "Reported" => Ok(Self::Reported),
            "Exiting" => Ok(Self::Exiting),
            "Retaining" => Ok(Self::Retaining),
            _ => Err(io::Error::other("unknown Git stand-in scenario")),
        }
    }
}
