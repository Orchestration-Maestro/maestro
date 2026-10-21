//! Completed exact-test subprocesses.

/// Complete the exact test with supplied environment presence and absence.
/// # Panics
/// Panics if the child cannot run or fails its assertions.
pub fn run_child(process: maestro_test_process::TestProcess, environment: &[(&str, Option<&str>)]) {
    let name = process.name().to_owned();
    let mut command = process.nocapture();
    for (key, value) in environment {
        if let Some(value) = value {
            command = command.env(key, value);
        } else {
            command = command.env_remove(key);
        }
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "isolated test failed: {name}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
