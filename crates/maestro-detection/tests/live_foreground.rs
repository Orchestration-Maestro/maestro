//! Recognition through live terminal process facts and the checked setup entry.
#![cfg(test)]
#![cfg(target_os = "linux")]

use maestro_detection::agents::{Agent, identify_agent_in_job};
use maestro_process::pty::{self, PtyCommand, PtyEnvironment, PtySize, foreground::ForegroundJob};
use maestro_test_process::{TestProcess, run_alone};
use std::{
    io::{self, Read, Write},
    process::{Command, Output},
};

#[test]
fn setup_entry() {
    let Some(script) = std::env::var_os("MAESTRO_DETECTION_SCRIPT") else {
        return;
    };
    let mut command = Command::new("/bin/bash");
    command
        .arg("-c")
        .arg(script)
        .env_remove("MAESTRO_DETECTION_SCRIPT");
    let _ = pty::setup_and_exec(command);
    std::process::exit(1);
}

/// Observes exec completion through process facts, then reaps the controlled terminal.
fn capture_job(script: &str, input: &[u8], release: &[u8], argv0: &str) -> ForegroundJob {
    let mut observed = None;
    TestProcess::named("live_foreground::setup_entry")
        .unwrap()
        .nocapture()
        .single_threaded()
        .output_with(|harness| {
            let command = PtyCommand {
                program: std::env::current_exe()?.into_os_string(),
                argv0: None,
                args: harness.iter().map(Into::into).collect(),
                cwd: None,
                environment: PtyEnvironment::Inherit(vec![(
                    "MAESTRO_DETECTION_SCRIPT".into(),
                    Some(script.into()),
                )]),
            };
            let (pty, mut child) = pty::spawn(
                PtySize {
                    rows: 24,
                    cols: 80,
                    cell_width_px: 0,
                    cell_height_px: 0,
                },
                command,
                Ok,
            )?;
            let mut master = std::fs::File::from(pty.master_fd);
            master.write_all(input)?;
            observed = Some(observe_sleep(&mut child, argv0)?);
            finish_terminal(&mut child, &mut master, release)
        })
        .unwrap();
    observed.unwrap()
}

/// Waits for the native exec observation rather than estimating readiness with a delay.
fn observe_sleep(child: &mut std::process::Child, argv0: &str) -> io::Result<ForegroundJob> {
    loop {
        if let Some(job) = pty::foreground::foreground_job(
            child.id(),
            pty::foreground::ProcessDetectionMode::Native,
        ) && sleep_exec_observed(&job, argv0)
        {
            return Ok(job);
        }
        if child.try_wait()?.is_some() {
            return Err(io::Error::other("terminal exited before exec observation"));
        }
        std::thread::yield_now();
    }
}

/// Reports whether the launched executable is present in the sampled job.
fn sleep_exec_observed(job: &ForegroundJob, argv0: &str) -> bool {
    job.processes.iter().any(|process| {
        process.name == "sleep"
            && process
                .argv
                .as_ref()
                .and_then(|words| words.first())
                .is_some_and(|word| word == argv0)
    })
}

#[test]
fn exec_observation_rejects_absent_and_stale_arguments() {
    let mut job = ForegroundJob {
        process_group_id: 1,
        processes: vec![pty::foreground::ForegroundProcess {
            pid: 1,
            name: "sleep".into(),
            argv0: None,
            argv: None,
            cmdline: None,
        }],
    };
    assert!(!sleep_exec_observed(&job, "codex"));
    job.processes[0].argv = Some(vec!["/bin/bash".into()]);
    assert!(!sleep_exec_observed(&job, "codex"));
    job.processes[0].argv = Some(vec!["codex".into(), "infinity".into()]);
    assert!(sleep_exec_observed(&job, "codex"));
}

/// Reaps the child and retains terminal bytes for the checked harness proof.
fn finish_terminal(
    child: &mut std::process::Child,
    master: &mut std::fs::File,
    release: &[u8],
) -> io::Result<Output> {
    if release.is_empty() {
        child.kill()?;
    } else {
        master.write_all(release)?;
    }
    let status = child.wait()?;
    let mut raw = Vec::new();
    match master.read_to_end(&mut raw) {
        Ok(_) => {}
        Err(error) if error.raw_os_error() == Some(5) => {}
        Err(error) => return Err(error),
    }
    assert!(raw.starts_with(b"\r\nrunning 1 test\r\n"));
    let stdout = maestro_text::normalize_to_lf(&String::from_utf8(raw).unwrap()).into_bytes();
    Ok(Output {
        status,
        stdout,
        stderr: vec![],
    })
}

#[test]
fn live_foreground_ordinary_program_has_no_agent() {
    if run_alone("live_foreground_ordinary_program_has_no_agent").unwrap() {
        return;
    }
    let job = capture_job("exec /bin/sleep infinity", b"", b"", "/bin/sleep");
    assert!(job.processes.iter().any(|process| process.name == "sleep"));
    assert_eq!(identify_agent_in_job(&job), None);
}

#[test]
fn live_shell_exec_replacement_has_no_agent() {
    if run_alone("live_shell_exec_replacement_has_no_agent").unwrap() {
        return;
    }
    let job = capture_job(
        "read -r command; eval \"$command\"",
        b"exec /bin/sleep infinity\n",
        b"",
        "/bin/sleep",
    );
    assert!(job.processes.iter().any(|process| process.name == "sleep"));
    assert_eq!(identify_agent_in_job(&job), None);
}

#[test]
fn live_wrapper_member_argv_identifies_an_agent() {
    if run_alone("live_wrapper_member_argv_identifies_an_agent").unwrap() {
        return;
    }
    let job = capture_job(
        "/bin/bash -c 'exec -a codex /bin/sleep infinity' & member=$!; read -r stop; kill \"$member\"; wait \"$member\"; exit 0",
        b"",
        b"stop\n",
        "codex",
    );
    assert!(job.processes.iter().any(|process| process.name == "bash"));
    assert!(job.processes.iter().any(|process| {
        process.name == "sleep"
            && process
                .argv
                .as_ref()
                .and_then(|argv| argv.first())
                .is_some_and(|argv0| argv0 == "codex")
    }));
    assert_eq!(
        identify_agent_in_job(&job),
        Some((Agent::Codex, "codex".into()))
    );
}
