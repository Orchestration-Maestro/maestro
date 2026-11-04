//! Browser executable configuration at the command boundary.

use std::process::Command;

/// Missing browser configuration fails before tool preparation or browser launch.
#[test]
fn browser_command_requires_chromium_path() {
    let output = Command::new(env!("CARGO_BIN_EXE_maestro-test-browser"))
        .env_remove("CHROMIUM_PATH")
        .output()
        .unwrap();

    let diagnostics = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{diagnostics}");
    assert!(diagnostics.contains("CHROMIUM_PATH"), "{diagnostics}");
    assert!(output.stdout.is_empty(), "{output:?}");
}
