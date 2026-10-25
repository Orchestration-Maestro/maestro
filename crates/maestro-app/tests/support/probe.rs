//! Reusable child-process test observation.
use maestro_test_process::TestProcess;
use std::path::Path;
#[cfg(unix)]
/// Run the configured test process with `envs` and
/// return the `PROBE ` lines the child printed.
/// Remove `removed` keys before applying `envs`; other keys remain inherited.
pub fn probe(
    process: TestProcess,
    current_dir: &Path,
    envs: &[(&str, &str)],
    removed: &[&str],
) -> Vec<String> {
    let mut command = process;
    for key in removed {
        command = command.env_remove(key);
    }
    let output = command
        .nocapture()
        .single_threaded()
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
