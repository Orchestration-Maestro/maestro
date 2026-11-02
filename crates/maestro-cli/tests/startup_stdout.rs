//! Protocol modes keep standard output empty while startup package chatter goes to stderr.
#![cfg(unix)]

use crate::support::entry::Entry;
use maestro_cli::startup::main::MainOptions;
use maestro_test_process::selection;
use serde_json::json;
use std::cell::RefCell;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// The two install lines the controlled package command reports.
const CHATTER: &str = "changed 1 package in 471ms\nfound 0 vulnerabilities\n";

/// Install the prepared native fixture as the configured package command.
fn package_command(directory: &Path) -> PathBuf {
    let artifact = std::env::var_os("MAESTRO_NATIVE_FIXTURE")
        .expect("run just prepare-tests, then run these tests with cargo nextest");
    let path = directory.join("controlled-npm");
    maestro_test_process::install_executable(Path::new(&artifact), &path).expect("install starts");
    let mut record = path.as_os_str().to_owned();
    record.push(".fixture");
    let scenario = json!({
        "scenario": "output",
        "stdout": CHATTER.as_bytes(),
        "stderr": [],
        "code": 0
    });
    std::fs::write(record, format!("command\n{scenario}")).expect("the record is writable");
    path
}

/// Ordinary output routed through the application's standard output owner.
struct Routed;

impl Write for Routed {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        maestro_app::output_guard::write_stdout(&String::from_utf8_lossy(bytes))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        io::stdout().flush()
    }
}

/// Run `args` over a configured, not yet installed package and check both process streams.
fn protocol_help(name: &str, args: &'static [&'static str]) {
    if selection().is_child() {
        let local = Rc::new(tokio::task::LocalSet::new());
        let runtime = crate::support::startup::runtime();
        let directory = maestro_test_directory::Scratch::new().unwrap();
        let npm = package_command(directory.path());
        runtime.block_on(local.run_until(async {
            let mut e = Entry::new();
            e.out = Rc::new(RefCell::new(Routed));
            e.err = Rc::new(RefCell::new(io::stderr()));
            e.settings(
                None,
                Some(json!({
                    "packages": ["npm:fake-package"],
                    "npmCommand": [npm.to_str().expect("the controlled command path is text")],
                })),
            );
            assert_eq!(
                e.run(&local, args, MainOptions::default()).await.unwrap(),
                0
            );
        }));
        return;
    }
    let root = maestro_test_directory::Scratch::new().unwrap();
    let artifact = std::env::var_os("MAESTRO_NATIVE_FIXTURE")
        .expect("run just prepare-tests, then run these tests with cargo nextest");
    let output = crate::support::missing_cwd::command(name, "protocol", root.path())
        .env("MAESTRO_NATIVE_FIXTURE", artifact)
        .quiet()
        .nocapture()
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let body = stdout.strip_prefix("\nrunning 1 test\n").unwrap();
    assert!(
        body.starts_with('.'),
        "protocol stdout was written: {body:?}"
    );
    for expected in [
        "changed 1 package in 471ms",
        "found 0 vulnerabilities",
        "Usage:",
    ] {
        assert!(stderr.contains(expected), "{expected}: {stderr}");
    }
}

#[test]
fn json_help_keeps_protocol_stdout_empty() {
    protocol_help(
        "startup_stdout::json_help_keeps_protocol_stdout_empty",
        &["--mode", "json", "--help"],
    );
}

#[test]
fn print_help_keeps_protocol_stdout_empty() {
    protocol_help(
        "startup_stdout::print_help_keeps_protocol_stdout_empty",
        &["-p", "--help"],
    );
}
