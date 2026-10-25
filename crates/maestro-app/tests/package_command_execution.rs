//! Selected self-updates planned and executed through the application owner.
#![cfg(test)]
#![cfg(unix)]

use crate::service_observations as services;
use crate::support::run_io;
use maestro_app::configuration::config::SelfUpdateOperations;
use maestro_app::configuration::config::{
    PACKAGE_NAME, SelfUpdateCommand, SelfUpdateCommandStep, VERSION, get_self_update_plan,
    run_self_update,
};
use maestro_app::configuration::version_check::VersionCheckOptions;
use maestro_process::SpawnSyncError;
use std::{fs, io, process::Output};

/// A read-only adapter that keeps the default inherited-I/O runner.
struct ReadOnly;

impl SelfUpdateOperations for ReadOnly {
    fn is_windows(&self) -> bool {
        false
    }
    fn run_command_sync(&self, _command: &str, _args: &[String]) -> Result<Output, SpawnSyncError> {
        Err(SpawnSyncError::Spawn(io::Error::other(
            "no synchronous query",
        )))
    }
}

/// One `/bin/sh -c` command with the selected steps.
fn shell_command(
    display: &str,
    script: &str,
    steps: Option<Vec<SelfUpdateCommandStep>>,
) -> SelfUpdateCommand {
    SelfUpdateCommand {
        command: "/bin/sh".into(),
        args: vec!["-c".into(), script.into()],
        display: display.into(),
        steps,
    }
}

/// One step appending `operand` as a line to `log`.
fn append_step(log: &str, operand: &str) -> SelfUpdateCommandStep {
    SelfUpdateCommandStep {
        command: "/bin/sh".into(),
        args: vec!["-c".into(), format!("printf '%s\\n' {operand} >> '{log}'")],
        display: format!("%s {operand}"),
    }
}

#[test]
fn self_update_runs_inherited_steps_without_deadline() {
    let scratch = crate::child::Scratch::new();
    let log = scratch.join("steps.log");
    let selected = shell_command(
        "single",
        &format!("printf '%s\\n' 'no steps' >> '{log}'"),
        Some(vec![
            append_step(&log, "\"$(pwd)\""),
            append_step(&log, "\"a b\""),
        ]),
    );
    run_io(run_self_update(&selected, &ReadOnly)).expect("the steps run in order");
    let recorded = fs::read_to_string(&log).expect("the steps wrote their log");
    let directory = std::env::current_dir().expect("the working directory is known");
    assert_eq!(
        recorded.lines().collect::<Vec<_>>(),
        [
            directory.to_str().expect("the working directory is text"),
            "a b"
        ]
    );
    let empty = shell_command("single", "exit 9", Some(Vec::new()));
    run_io(run_self_update(&empty, &ReadOnly)).expect("no steps do no work");
    assert_eq!(
        fs::read_to_string(&log).expect("the log is readable"),
        recorded
    );
}

/// The message of `command` failing under the default runner.
fn failure(command: &SelfUpdateCommand) -> String {
    run_io(run_self_update(command, &ReadOnly))
        .expect_err("the step fails")
        .to_string()
}

#[test]
fn self_update_step_errors_keep_status_and_signal() {
    let scratch = crate::child::Scratch::new();
    let log = scratch.join("steps.log");
    let absent = SelfUpdateCommand {
        command: scratch.join("absent-program"),
        args: vec!["install".into(), "maestro".into()],
        display: "absent-program".into(),
        steps: None,
    };
    assert_eq!(
        run_io(run_self_update(&absent, &ReadOnly))
            .expect_err("the executable is absent")
            .kind(),
        io::ErrorKind::NotFound
    );
    assert_eq!(
        failure(&shell_command("coded", "exit 23", None)),
        "coded exited with code 23"
    );
    assert_eq!(
        failure(&shell_command("signalled", "kill -TERM $$", None)),
        "signalled terminated by signal SIGTERM"
    );
    #[cfg(target_os = "linux")]
    assert_eq!(
        failure(&shell_command("realtime", "kill -34 $$", None)),
        "realtime exited with code unknown"
    );
    let failing = shell_command(
        "failing",
        "exit 3",
        Some(vec![
            SelfUpdateCommandStep {
                command: "/bin/sh".into(),
                args: vec!["-c".into(), "exit 3".into()],
                display: "%s".into(),
            },
            append_step(&log, "'later'"),
        ]),
    );
    assert_eq!(failure(&failing), "%s exited with code 3");
    assert!(!std::path::Path::new(&log).exists());
}

/// The plan of one controlled release, with the requests it made.
fn planned(
    force: bool,
    origin: &str,
    status: u16,
    body: &str,
    mode: &str,
) -> (String, bool, usize) {
    let transport = services::Transport::new(status, body.into(), mode);
    let options = VersionCheckOptions {
        timeout_ms: None,
        fetch: Some(transport.fetch.clone()),
        offline: None,
        skip_version_check: None,
    };
    let plan = run_io(get_self_update_plan(force, origin, &options));
    (plan.package_name, plan.should_run, transport.count())
}

#[test]
fn self_update_plan_retains_lookup_fallbacks() {
    if services::isolated("self_update_plan_retains_lookup_fallbacks") {
        return;
    }
    assert_eq!(
        planned(true, services::ORIGIN, 200, "not read", ""),
        (PACKAGE_NAME.into(), true, 0)
    );
    assert_eq!(
        planned(false, "", 200, "not read", ""),
        (PACKAGE_NAME.into(), true, 0)
    );
    assert_eq!(
        planned(false, services::ORIGIN, 200, "", "FETCH_ERROR"),
        (PACKAGE_NAME.into(), true, 1)
    );
    assert_eq!(
        planned(false, services::ORIGIN, 404, "", ""),
        (PACKAGE_NAME.into(), true, 1)
    );
    assert_eq!(
        planned(false, services::ORIGIN, 200, r#"{"version":"99.0.0"}"#, ""),
        (PACKAGE_NAME.into(), true, 1)
    );
    assert_eq!(
        planned(
            false,
            services::ORIGIN,
            200,
            r#"{"version":"0.0.1","packageName":"maestro-next"}"#,
            ""
        ),
        ("maestro-next".into(), true, 1)
    );
    assert_eq!(
        planned(
            false,
            services::ORIGIN,
            200,
            &format!(r#"{{"version":"{VERSION}"}}"#),
            ""
        ),
        (PACKAGE_NAME.into(), false, 1)
    );
    assert_eq!(
        planned(false, services::ORIGIN, 200, r#"{"version":"0.0.1"}"#, ""),
        (PACKAGE_NAME.into(), false, 1)
    );
    assert_eq!(
        planned(
            false,
            services::ORIGIN,
            200,
            r#"{"version":"unparsed"}"#,
            ""
        ),
        (PACKAGE_NAME.into(), true, 1)
    );
}
