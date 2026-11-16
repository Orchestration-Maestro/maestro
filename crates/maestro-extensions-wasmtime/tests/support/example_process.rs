//! Local executors and prepared native fixtures for native component tests.
#![cfg(test)]
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::path::{Path, PathBuf};

/// Runs native process futures on their caller-owned local executor.
pub use maestro_test_process::executor::run;

/// Installs the prepared native fixture as the program `name` in `directory` and records the
/// `family` scenario it plays; returns the program's path.
pub fn install(
    directory: &Path,
    name: &str,
    family: &str,
    scenario: &serde_json::Value,
) -> PathBuf {
    let artifact = std::env::var_os("MAESTRO_NATIVE_FIXTURE")
        .expect("run just prepare-tests, then run these tests with cargo nextest");
    let program = directory.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    maestro_test_process::install_executable(Path::new(&artifact), &program)
        .expect("the fixture installs");
    record(&program, family, scenario);
    program
}

/// Records the `family` scenario the installed fixture `program` plays.
pub fn record(program: &Path, family: &str, scenario: &serde_json::Value) {
    let mut record = program.as_os_str().to_owned();
    record.push(".fixture");
    std::fs::write(record, format!("{family}\n{scenario}")).expect("the record is writable");
}
