//! A caller that spawns the test executable through its own product code takes the
//! prepared command from the checked process.
#![cfg(test)]

use maestro_test_process::{HARNESS_START, TestProcess, test_name};

/// The ignored probe the command re-runs; `MTP_BEHAVIOR` selects what it does.
#[test]
#[ignore = "runs only inside a controlled child"]
fn command_probe() {
    crate::behave("command");
}

/// A checked builder for the probe with `behavior` selected.
fn probe(behavior: &str) -> TestProcess {
    TestProcess::child(command_probe)
        .unwrap()
        .env("MTP_BEHAVIOR", behavior)
}

#[test]
fn command_carries_the_builder_settings_to_the_caller() {
    let output = probe("echo")
        .command()
        .unwrap()
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();

    assert!(output.stdout.starts_with(HARNESS_START));
    assert!(output.stdout[HARNESS_START.len()..].starts_with(b"stdin:"));
    assert_eq!(output.stderr, b"err\0\xff");
}

#[test]
fn command_rejects_a_stale_identity() {
    let error = TestProcess::named("command_route::stale_name")
        .unwrap()
        .ignored()
        .command()
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("lists no test with this exact identity"),
        "{error}"
    );
}

#[test]
fn child_selects_an_ignored_test_by_item_and_prints_its_output() {
    let child = TestProcess::child(command_probe).unwrap();

    assert_eq!(child.name(), test_name(command_probe).unwrap());
    let output = child.env("MTP_BEHAVIOR", "print").output().unwrap();
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("printed command\n"),
        "{output:?}"
    );
}
