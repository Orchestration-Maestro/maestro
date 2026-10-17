//! Completed exact-test subprocesses.
#[allow(
    dead_code,
    reason = "Child runs use only test selection and the execution check."
)]
#[path = "../../../maestro-models/tests/support/child_process.rs"]
pub mod child_process;

/// Complete the exact test with supplied environment presence and absence.
/// # Panics
/// Panics if the child cannot run or fails its assertions.
pub fn run_child(name: &str, environment: &[(&str, Option<&str>)]) {
    let name = child_process::selector(name).unwrap();
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", &name, "--nocapture"])
        .env("MAESTRO_DYNAMIC_CHILD", "1");
    for (key, value) in environment {
        if let Some(value) = value {
            command.env(key, value);
        } else {
            command.env_remove(key);
        }
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "isolated test failed: {name}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    child_process::assert_executed(&output, &name);
}
