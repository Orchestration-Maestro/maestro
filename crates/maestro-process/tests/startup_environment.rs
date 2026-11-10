//! Startup marker defaults through the existing native process owners.
#![cfg(test)]
#![cfg(unix)]
use crate::child_programs;
use maestro_process::commands::Launch;
use maestro_process::{OutputMode, SpawnSyncOptions, spawn_sync};
use maestro_test_process::child::without_harness;
use std::path::Path;

/// A launch of `program` from this test executable, with `operands` after the harness
/// arguments.
fn launch(program: impl Fn(), operands: &[&str]) -> Launch {
    let command = child_programs::command(program, &[]);
    let mut args: Vec<String> = command
        .get_args()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    args.extend(operands.iter().map(|operand| (*operand).to_owned()));
    Launch {
        executable: command.get_program().to_string_lossy().into_owned(),
        args,
        verbatim: false,
    }
}

/// A launch of the environment probe, which prints the marker, an unrelated entry, one
/// literal argument and its working directory.
fn probe() -> Launch {
    launch(child_programs::environment_probe, &["literal $HOME"])
}

/// Captured stdout of `launch` over `environment` in `cwd`.
fn output(launch: Launch, environment: &[(&str, &str)], cwd: Option<&Path>) -> String {
    let output = launch
        .with_context(environment.iter().copied(), cwd)
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(without_harness(&output.stdout)).unwrap()
}

#[test]
fn startup_child_default_keeps_explicit_marker_overlay() {
    let tree = maestro_test_directory::Scratch::new().unwrap();
    let observed = output(
        probe(),
        &[
            ("MAESTRO_CODING_AGENT", "true"),
            ("KEPT", "kept"),
            ("MAESTRO_CODING_AGENT", "false"),
        ],
        Some(tree.path()),
    );
    assert_eq!(
        observed,
        format!("false|kept|literal $HOME|{}", tree.path().display())
    );
}

/// Captured stdout of a command whose environment the caller edited.
///
/// The fixture supplies the unrelated `ONLY` itself and then removes it, so an edit that
/// sets the key still shows through while the caller's own `ONLY` value cannot leak into
/// the child.
fn edited(edit: impl FnOnce(&mut tokio::process::Command)) -> String {
    let mut command = child_programs::async_command(child_programs::marker_probe, &[]);
    command.env("MAESTRO_CODING_AGENT", "true");
    command.env("ONLY", "ambient");
    command.env_remove("ONLY");
    edit(&mut command);
    let output = spawn_sync(
        &mut command,
        SpawnSyncOptions::Capture {
            max_buffer_bytes: None,
            stdout: OutputMode::Capture,
            input: None,
            timeout: None,
            stderr: OutputMode::Discard,
        },
    )
    .unwrap();
    String::from_utf8_lossy(&without_harness(&output.stdout)).into_owned()
}

#[test]
fn startup_child_default_keeps_marker_removal_and_replacement() {
    assert_eq!(
        edited(|command| {
            command.env_remove("MAESTRO_CODING_AGENT");
        }),
        "|"
    );
    assert_eq!(
        edited(|command| {
            command.env_clear();
            command.env("ONLY", "replacement");
        }),
        "|replacement"
    );
    assert_eq!(
        edited(|command| {
            command.env("MAESTRO_CODING_AGENT", "edited");
        }),
        "edited|"
    );
}

/// One ambient marker case: the value the parent supplies, and the marker the child sees.
const AMBIENT: [(Option<&str>, &str); 2] = [(Some("true"), "true"), (None, "")];

#[test]
fn sdk_children_keep_ambient_marker() {
    let selection = maestro_test_process::selection();
    if !selection.is_child() {
        for (index, (value, _observed)) in AMBIENT.iter().enumerate() {
            let case = index.to_string();
            let mut child =
                maestro_test_process::TestProcess::current("sdk_children_keep_ambient_marker")
                    .unwrap()
                    .case(&case);
            child = match value {
                Some(value) => child.env("MAESTRO_CODING_AGENT", value),
                None => child.env_remove("MAESTRO_CODING_AGENT"),
            };
            child = child.env("KEPT", "ambient").env_remove("KEPT");
            let output = child.output().unwrap();
            assert!(output.status.success(), "{case}");
        }
        return;
    }
    let case = (0..AMBIENT.len())
        .map(|index| index.to_string())
        .find(|case| selection.runs_case(case))
        .expect("selected ambient case");
    let observed = AMBIENT[case.parse::<usize>().unwrap()].1;
    let directory = std::env::current_dir().unwrap();
    let expected = format!("{observed}||literal $HOME|{}", directory.display());
    assert_eq!(output(probe(), &[], None), expected);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let captured = runtime.block_on(async {
        let mut child = probe()
            .with_context(std::iter::empty::<(&str, &str)>(), None)
            .spawn_captured()
            .unwrap();
        maestro_process::children::wait_for_child_process(
            &mut child,
            maestro_process::children::WaitOptions::default(),
            std::future::pending(),
            None,
        )
        .await
        .unwrap()
    });
    assert_eq!(
        String::from_utf8(without_harness(&captured.stdout)).unwrap(),
        expected
    );
}

#[test]
fn startup_child_default_keeps_launch_context() {
    let captured = probe()
        .with_context([("MAESTRO_CODING_AGENT", "true")], None)
        .output()
        .unwrap();
    assert_eq!(captured.status.code(), Some(0));
    assert_eq!(captured.stderr, Vec::<u8>::new());
    assert_eq!(
        output(launch(child_programs::input_state, &[]), &[], None),
        "eof"
    );
    let status = launch(child_programs::output_writer, &[])
        .with_context(
            [("MAESTRO_CODING_AGENT", "true"), ("CHILD_EXIT", "7")],
            None,
        )
        .output()
        .unwrap();
    assert_eq!(status.status.code(), Some(7));
    let missing = Launch {
        executable: "/nonexistent/maestro-child".into(),
        args: Vec::new(),
        verbatim: false,
    }
    .with_context([("MAESTRO_CODING_AGENT", "true")], None)
    .output();
    assert!(missing.is_err());
}

#[test]
fn executable_marker_default_precedes_explicit_child_edits() {
    if !maestro_test_process::selection().is_child() {
        let output = maestro_test_process::TestProcess::current(
            "startup_environment::executable_marker_default_precedes_explicit_child_edits",
        )
        .unwrap()
        .env_clear()
        .case("marker")
        .output()
        .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    maestro_process::initialize_coding_agent();
    for (edit, expected) in [
        (None, "true"),
        (Some(None), ""),
        (Some(Some("custom")), "custom"),
    ] {
        let fixture = child_programs::command(child_programs::marker_value, &[]);
        let mut command =
            maestro_process::command(fixture.get_program().to_string_lossy().as_ref());
        command.args(fixture.get_args());
        match edit {
            Some(None) => {
                command.env_remove("MAESTRO_CODING_AGENT");
            }
            Some(Some(value)) => {
                command.env("MAESTRO_CODING_AGENT", value);
            }
            None => {}
        }
        let output = spawn_sync(
            &mut command,
            SpawnSyncOptions::Capture {
                max_buffer_bytes: None,
                stdout: OutputMode::Capture,
                stderr: OutputMode::Capture,
                input: None,
                timeout: None,
            },
        )
        .unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(without_harness(&output.stdout)).unwrap(),
            expected
        );
    }
}
