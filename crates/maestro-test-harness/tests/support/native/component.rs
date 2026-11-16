//! Programs a guest component runs through explicit execution, selected by a sibling JSON record.
//!
//! The record names the scenario; the component's literal operands carry the per-call values.

use std::ffi::OsString;
use std::io;

use serde::Deserialize;

/// Component execution scenarios, parsed completely before any file effect.
#[derive(Deserialize)]
#[serde(tag = "scenario", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Record {
    /// Create the file the only operand names.
    Touch {},
    /// Replace this process with a test binary rerunning one test.
    #[cfg(unix)]
    Rerun {
        /// The test executable to run.
        binary: String,
        /// The exact test name it runs.
        test: String,
    },
}

impl Record {
    /// Reject unknown scenarios and fields before effects.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let json = text
            .strip_prefix("component-exec\n")
            .ok_or_else(|| io::Error::other("missing component-exec family"))?;
        serde_json::from_str(json).map_err(io::Error::other)
    }
}

/// Run the selected program with its untouched native operands.
pub(super) fn run(record: &Record, operands: &[OsString]) -> io::Result<()> {
    match record {
        Record::Touch {} => {
            let [path] = operands else {
                return Err(io::Error::other("expected one path"));
            };
            std::fs::File::create(path).map(drop)
        }
        #[cfg(unix)]
        Record::Rerun { binary, test } => {
            use std::os::unix::process::CommandExt;
            Err(std::process::Command::new(binary)
                .args([test, "--exact", "--nocapture"])
                .exec())
        }
    }
}
