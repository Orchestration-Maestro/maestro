//! Controlled native programs for the interactive mode's process witnesses.

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::Path;

use serde::Deserialize;

mod notices;
mod share;
mod shell;
mod signals;

/// Interactive-mode scenarios, parsed completely before any file, socket or stream effect.
#[derive(Deserialize)]
#[serde(tag = "scenario", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Record {
    /// Run the recorded steps as the configured shell would run a command.
    Shell {
        /// Ordered effects, ending the process after the last.
        steps: Vec<shell::Step>,
    },
    /// Answer a terminal multiplexer option query with the recorded results.
    Tmux {
        /// Result for `extended-keys`.
        extended_keys: notices::Reply,
        /// Result for `extended-keys-format`.
        extended_keys_format: notices::Reply,
    },
    /// Answer a terminal multiplexer option query after the test's permit.
    GatedTmux {
        /// Directory holding the `.ready` and `.permit` pipes.
        directory: String,
    },
    /// Answer a package version query after the test's permit.
    GatedPackage {
        /// Directory holding the `.ready` and `.permit` pipes.
        directory: String,
    },
    /// Answer a search program's version probe after the test's permit.
    GatedProbe {
        /// Directory holding the `.ready` and `.permit` pipes.
        directory: String,
    },
    /// Answer a configured package-manager version query and log it.
    PackageVersion {
        /// File each invocation appends its directory and operands to.
        log: String,
    },
    /// Answer the Git queries of a package update check and log them.
    PackageRemote {
        /// File each invocation appends one JSON observation to.
        log: String,
    },
    /// Stand in for the GitHub command line.
    Gh(share::Gh),
    /// Wait until a signal ends the process.
    Pause {},
    /// Report readiness, then report a stop signal or end when input arrives.
    GroupMember {},
}

impl Record {
    /// Reject unknown scenarios and fields before effects.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let json = text
            .strip_prefix("chat\n")
            .ok_or_else(|| io::Error::other("missing chat family"))?;
        serde_json::from_str(json).map_err(io::Error::other)
    }
}

/// Append `bytes` to the log at `path`, creating it first.
fn append(path: impl AsRef<Path>, bytes: &[u8]) -> io::Result<()> {
    std::fs::File::options()
        .append(true)
        .create(true)
        .open(path)?
        .write_all(bytes)
}

/// Run the selected program with its untouched native operands.
pub(super) fn run(record: &Record, operands: &[OsString]) -> io::Result<()> {
    match record {
        Record::Shell { steps } => shell::run(steps, operands),
        Record::Tmux {
            extended_keys,
            extended_keys_format,
        } => notices::tmux(extended_keys, extended_keys_format, operands),
        Record::GatedTmux { directory } => notices::gated_tmux(directory, operands),
        Record::GatedPackage { directory } => notices::gated_package(directory, operands),
        Record::GatedProbe { directory } => notices::gated_probe(directory, operands),
        Record::PackageVersion { log } => notices::package_version(log, operands),
        Record::PackageRemote { log } => notices::package_remote(log, operands),
        Record::Gh(gh) => share::run(gh, operands),
        Record::Pause {} => signals::pause(),
        Record::GroupMember {} => signals::group_member(),
    }
}
