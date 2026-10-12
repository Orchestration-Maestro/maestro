//! Completed exact-test subprocesses.
/// Complete the exact test with supplied environment presence and absence.
/// # Panics
/// Panics if the child cannot run or fails its assertions.
pub fn run_child(name: &str, environment: &[(&str, Option<&str>)]) {
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", name, "--nocapture"])
        .env("MAESTRO_DYNAMIC_CHILD", "1");
    for (key, value) in environment {
        if let Some(value) = value {
            command.env(key, value);
        } else {
            command.env_remove(key);
        }
    }
    let status = command.status().unwrap();
    assert!(status.success(), "isolated test failed: {name}");
}
