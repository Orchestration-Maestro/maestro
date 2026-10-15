//! Reusable child-process test observation.
use std::{path::Path, process::Command};
#[cfg(unix)]
/// Run this test binary again for the single test `name`, with `envs`, and
/// return the `PROBE ` lines the child printed.
/// Remove `removed` keys before applying `envs`; other keys remain inherited.
pub fn probe(
    name: &str,
    current_dir: &Path,
    envs: &[(&str, &str)],
    removed: &[&str],
) -> Vec<String> {
    let mut command = Command::new(std::env::current_exe().expect("test binary path"));
    for key in removed {
        command.env_remove(key);
    }
    let output = command
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .current_dir(current_dir)
        .envs(envs.iter().copied())
        .output()
        .expect("child starts");
    assert!(
        output.status.success(),
        "child probe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.strip_prefix("PROBE ").map(str::to_owned))
        .collect()
}
