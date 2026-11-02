//! Compiler wrappers for fixture builds, installed from the prepared artifact.

use std::ffi::OsString;
use std::io::{self, Write};
use std::process::Command;

use maestro_test_process::child::leave;
use serde::Deserialize;

/// A wrapper's complete behavior, parsed before it runs a child.
#[derive(Deserialize)]
#[serde(tag = "scenario", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Record {
    /// Drop the fixture's Cargo home and run the wrapper this one replaced.
    HideHome,
    /// Record the Cargo home the compiler saw, then run the compiler.
    Recording,
}

impl Record {
    /// Read a tooling-wrapper record, rejecting unknown scenarios and fields.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let json = text
            .strip_prefix("tooling-wrapper\n")
            .ok_or_else(|| io::Error::other("missing tooling-wrapper family"))?;
        serde_json::from_str(json).map_err(io::Error::other)
    }
}

/// Run the selected wrapper with its untouched native operands.
pub(super) fn run(record: &Record, operands: &[OsString]) -> io::Result<()> {
    match record {
        Record::HideHome => {
            let wrapper = std::env::var_os("MAESTRO_FIXTURE_RUSTC_WRAPPER")
                .ok_or_else(|| io::Error::other("the replaced wrapper is unset"))?;
            forward(
                Command::new(wrapper)
                    .args(operands)
                    .env_remove("CARGO_HOME"),
            )
        }
        Record::Recording => {
            let mut log = std::env::current_exe()?.into_os_string();
            log.push(".log");
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log)?;
            writeln!(
                file,
                "{}",
                std::env::var("CARGO_HOME").unwrap_or_else(|_| "unset".to_owned())
            )?;
            let (compiler, arguments) = operands
                .split_first()
                .ok_or_else(|| io::Error::other("missing compiler operand"))?;
            forward(Command::new(compiler).args(arguments))
        }
    }
}

/// Run the selected compiler or wrapper and take its status.
fn forward(command: &mut Command) -> io::Result<()> {
    let status = command.status()?;
    leave(status.code().unwrap_or(1))
}
