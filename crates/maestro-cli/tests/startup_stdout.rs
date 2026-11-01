//! Protocol modes keep standard output empty while startup package chatter goes to stderr.
#![cfg(unix)]

use crate::support::entry::Entry;
use maestro_cli::startup::main::MainOptions;
use maestro_test_process::selection;
use serde_json::json;
use std::cell::RefCell;
use std::io::{self, Write};
use std::rc::Rc;

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
        runtime.block_on(local.run_until(async {
            let mut e = Entry::new();
            e.out = Rc::new(RefCell::new(Routed));
            e.err = Rc::new(RefCell::new(io::stderr()));
            let chatter = "echo 'changed 1 package in 471ms'; echo 'found 0 vulnerabilities'";
            e.settings(
                None,
                Some(json!({
                    "packages": ["npm:fake-package"],
                    "npmCommand": ["/bin/sh", "-c", chatter],
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
    let output = crate::support::missing_cwd::command(name, "protocol", root.path())
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
