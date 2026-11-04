//! Git fixtures and local executors for native component examples.
#![cfg(test)]

use std::{
    path::{Path, PathBuf},
    process::Command,
};

/// Runs Git only inside an exclusively owned disposable repository, with the environment
/// the example's own Git commands inherit.
pub fn git(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(path)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// Runs the calling test again in a child whose Git commands, the example's included, read no
/// system or global configuration; returns `true` in the parent once that child passed.
pub fn isolated(test: &str) -> bool {
    if maestro_test_process::selection().is_child() {
        return false;
    }
    let status = maestro_test_process::TestProcess::current(test)
        .unwrap()
        .case("*")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .nocapture()
        .status()
        .unwrap();
    assert!(status.success(), "{test}: {status}");
    true
}

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
