//! Platform-independent foreground observations supplied to terminal policies.

use maestro_process::foreground::{ForegroundJob, ForegroundProcess};

/// Foreground facts remain usable without importing a platform-specific PTY.
#[test]
fn foreground_observation_preserves_process_facts() {
    let process = ForegroundProcess {
        pid: 42,
        name: "shell".into(),
        argv0: Some("/bin/shell".into()),
        argv: Some(vec!["shell".into(), "-l".into()]),
        cmdline: Some("shell -l".into()),
    };
    let job = ForegroundJob {
        process_group_id: 42,
        processes: vec![process.clone()],
    };
    assert_eq!(job.process_group_id, 42);
    assert_eq!(job.processes, vec![process]);
}
