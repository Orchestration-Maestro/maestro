//! Completed exact-test subprocesses.
#![cfg(not(target_arch = "wasm32"))]

/// Complete the exact test with supplied environment presence and absence.
/// # Errors
/// Returns failures starting or collecting the checked child.
///
/// # Panics
/// Panics if the child cannot run or fails its assertions.
pub fn run_child(
    process: crate::TestProcess,
    environment: &[(&str, Option<&str>)],
) -> Result<(), crate::TestProcessError> {
    let name = process.name().to_owned();
    let mut command = process.nocapture();
    for (key, value) in environment {
        if let Some(value) = value {
            command = command.env(key, value);
        } else {
            command = command.env_remove(key);
        }
    }
    let output = command.output()?;
    assert!(
        output.status.success(),
        "isolated test failed: {name}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
