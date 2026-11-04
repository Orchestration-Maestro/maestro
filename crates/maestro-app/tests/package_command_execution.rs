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
use serde_json::{Value, json};
use std::{fs, io, path::Path, process::Output};

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

/// One selected command whose stand-in reports `record`.
fn command(directory: &Path, name: &str, display: &str, record: &Value) -> SelfUpdateCommand {
    SelfUpdateCommand {
        command: crate::native::command(directory, name, record)
            .to_str()
            .expect("the stand-in path is text")
            .into(),
        args: Vec::new(),
        display: display.into(),
        steps: None,
    }
}

/// One selected step whose stand-in reports `record`.
fn step(directory: &Path, name: &str, record: &Value) -> SelfUpdateCommandStep {
    let step = command(directory, name, "%s", record);
    SelfUpdateCommandStep {
        command: step.command,
        args: step.args,
        display: step.display,
    }
}

/// A stand-in appending `bytes`, after the working directory when `cwd` is set.
fn appending(log: &Path, cwd: bool, bytes: &[u8]) -> Value {
    json!({
        "scenario": "output",
        "stdout": [],
        "stderr": [],
        "code": 0,
        "append": {"path": log, "cwd": cwd, "bytes": bytes}
    })
}

#[test]
fn self_update_runs_inherited_steps_without_deadline() {
    let scratch = crate::child::Scratch::new();
    let root = scratch.join("");
    let directory = Path::new(&root);
    let log = Path::new(&scratch.join("steps.log")).to_path_buf();
    let selected = SelfUpdateCommand {
        steps: Some(vec![
            step(directory, "first-step", &appending(&log, true, &[])),
            step(directory, "second-step", &appending(&log, false, b"a b\n")),
        ]),
        ..command(
            directory,
            "selected",
            "single",
            &appending(&log, false, b"no steps\n"),
        )
    };
    run_io(run_self_update(&selected, &ReadOnly)).expect("the steps run in order");
    let recorded = fs::read_to_string(&log).expect("the steps wrote their log");
    let cwd = std::env::current_dir().expect("the working directory is known");
    assert_eq!(
        recorded.lines().collect::<Vec<_>>(),
        [cwd.to_str().expect("the working directory is text"), "a b"]
    );
    let empty = SelfUpdateCommand {
        steps: Some(Vec::new()),
        ..command(
            directory,
            "empty",
            "single",
            &json!({"scenario": "output", "stdout": [], "stderr": [], "code": 9}),
        )
    };
    run_io(run_self_update(&empty, &ReadOnly)).expect("no steps do no work");
    assert_eq!(
        fs::read_to_string(&log).expect("the log is readable"),
        recorded
    );
}

/// A step the default inherited runner executes once, observed in the scratch root.
fn executed_step(directory: &Path, log: &Path, operands_log: &Path) -> SelfUpdateCommandStep {
    SelfUpdateCommandStep {
        args: vec!["operand".into()],
        ..step(
            directory,
            "executed-step",
            &json!({
                "scenario": "output",
                "stdout": b"inherited-stdout\n".to_vec(),
                "stderr": b"inherited-stderr\n".to_vec(),
                "code": 0,
                "append": {
                    "path": log,
                    "cwd": false,
                    "variable": "MAESTRO_NATIVE_FIXTURE",
                    "bytes": []
                },
                "argv_log": {"path": operands_log, "mark": "launch"}
            }),
        )
    }
}

#[test]
fn inherited_steps_run_authored_operands_with_the_parent_environment() {
    if maestro_test_process::selection().is_child() {
        inherited_steps_run_authored_operands_child();
        return;
    }
    let output = maestro_test_process::TestProcess::current(
        "inherited_steps_run_authored_operands_with_the_parent_environment",
    )
    .expect("the checked child runs")
    .nocapture()
    .case("*")
    .output()
    .expect("the checked child finishes");
    assert!(
        output.status.success(),
        "the child body failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let frames = [
        (&output.stdout, &b"inherited-stdout\n"[..]),
        (&output.stderr, &b"inherited-stderr\n"[..]),
    ];
    for (stream, frame) in frames {
        assert_eq!(
            stream.windows(frame.len()).filter(|w| *w == frame).count(),
            1,
            "the step's inherited stream reaches the checked child"
        );
    }
}

/// The parent environment and the inherited streams, as the step's stand-in reports them.
fn inherited_steps_run_authored_operands_child() {
    let scratch = crate::child::Scratch::new();
    let root = scratch.join("");
    let directory = Path::new(&root);
    let log = Path::new(&scratch.join("steps.log")).to_path_buf();
    let operands = Path::new(&scratch.join("operands.log")).to_path_buf();
    let artifact = std::env::var("MAESTRO_NATIVE_FIXTURE")
        .expect("the prepared native fixture runs these tests");
    let selected = SelfUpdateCommand {
        steps: Some(vec![executed_step(directory, &log, &operands)]),
        ..command(
            directory,
            "inherited",
            "single",
            &appending(&log, false, &[]),
        )
    };

    run_io(run_self_update(&selected, &ReadOnly)).expect("the step runs in place");

    assert_eq!(
        fs::read_to_string(&operands).expect("the run wrote its operands"),
        "launch\0operand\0",
        "the step receives the authored command and arguments untouched"
    );
    assert_eq!(
        fs::read_to_string(&log).expect("the run wrote its environment"),
        format!("MAESTRO_NATIVE_FIXTURE={artifact}\n"),
        "the step inherits the parent environment unchanged"
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
    let root = scratch.join("");
    let directory = Path::new(&root);
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
    let coded = command(
        directory,
        "coded",
        "coded",
        &json!({"scenario": "output", "stdout": [], "stderr": [], "code": 23}),
    );
    assert_eq!(failure(&coded), "coded exited with code 23");
    let signalled = command(
        directory,
        "signalled",
        "signalled",
        &json!({
            "scenario": "output",
            "stdout": [],
            "stderr": [],
            "code": 0,
            "signal": "SIGTERM"
        }),
    );
    assert_eq!(
        failure(&signalled),
        "signalled terminated by signal SIGTERM"
    );
    #[cfg(target_os = "linux")]
    assert_eq!(
        failure(&command(
            directory,
            "realtime",
            "realtime",
            &json!({
                "scenario": "output",
                "stdout": [],
                "stderr": [],
                "code": 0,
                "signal": "SIGRTMAX"
            }),
        )),
        "realtime exited with code unknown"
    );
    let failing = failing_steps(directory, Path::new(&log));
    assert_eq!(failure(&failing), "%s exited with code 3");
    assert!(!Path::new(&log).exists());
}

/// A selected command whose first step fails and whose second must not run.
fn failing_steps(directory: &Path, log: &Path) -> SelfUpdateCommand {
    SelfUpdateCommand {
        steps: Some(vec![
            step(
                directory,
                "failing-step",
                &json!({"scenario": "output", "stdout": [], "stderr": [], "code": 3}),
            ),
            step(directory, "later-step", &appending(log, false, b"later\n")),
        ]),
        ..command(
            directory,
            "failing",
            "%s",
            &json!({"scenario": "output", "stdout": [], "stderr": [], "code": 3}),
        )
    }
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
