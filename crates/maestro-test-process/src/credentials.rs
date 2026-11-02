//! Installation of the prepared credential-process aliases.

use std::io;
use std::path::Path;

/// Install an alias selected by `scenario` from the prepared `MAESTRO_NATIVE_FIXTURE` artifact.
///
/// The caller owns `entry` and its sibling `.fixture` record; the parent directory must already
/// exist. An AWS credential process counts its runs in `process-count` beside the alias.
///
/// # Errors
///
/// Returns an error when repository preparation is missing, installation fails or the record
/// cannot be written.
///
/// # Examples
///
/// ```no_run
/// # fn main() -> std::io::Result<()> {
/// use maestro_test_process::credentials::{Scenario, install};
/// let directory = maestro_test_directory::Scratch::new()?;
/// let process = directory.path().join(format!("process{}", std::env::consts::EXE_SUFFIX));
/// install(&process, Scenario::AwsProcess)?;
/// # Ok(())
/// # }
/// ```
pub fn install(entry: &Path, scenario: Scenario) -> io::Result<()> {
    let artifact = std::env::var_os("MAESTRO_NATIVE_FIXTURE")
        .ok_or_else(|| io::Error::other("run just prepare-tests, then cargo nextest"))?;
    crate::install_executable(Path::new(&artifact), entry)?;
    let mut record = entry.as_os_str().to_owned();
    record.push(".fixture");
    std::fs::write(record, format!("credentials\n{}\n", scenario.name()))
}

/// The credential protocol an installed alias serves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    /// An AWS credential process, whose variant is the alias's single operand.
    AwsProcess,
    /// A Google external account executable source, which takes no operand.
    GoogleExecutable,
}

impl Scenario {
    /// The exact selector written into the alias's sibling record.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::AwsProcess => "AwsProcess",
            Self::GoogleExecutable => "GoogleExecutable",
        }
    }
}
