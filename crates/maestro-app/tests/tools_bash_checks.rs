//! Raw shell output, status and artifact policy through application exports.
#![cfg(not(target_arch = "wasm32"))]

use crate::support::{
    io_runtime, run_io,
    session_harness::{
        ScriptedShell, ShellFailure, as_recorded, bash_result_view, case, on_paused_task_set,
    },
};
use maestro_app::{BashToolOptions, create_bash_tool};
use serde_json::json;
use std::{
    fmt::Write as _,
    sync::{Arc, Mutex},
};

/// A root-exported executable tool over supplied shell observations.
fn scripted(shell: ScriptedShell) -> maestro_agent::AgentTool {
    create_bash_tool(
        "/work".into(),
        String::new(),
        BashToolOptions {
            operations: Some(Arc::new(shell)),
            ..BashToolOptions::default()
        },
    )
}

/// Runs artifact cases in a checked Rust child; returns true when the parent has finished.
fn run_artifact_child(test: &str) -> bool {
    if maestro_test_process::selection().is_child() {
        return false;
    }
    let root = maestro_test_directory::Scratch::new().unwrap();
    let output = maestro_test_process::TestProcess::current(test)
        .unwrap()
        .case("*")
        .nocapture()
        .temp_dir(root.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    true
}

/// Reads only a real artifact within this child's disposable temporary directory.
fn artifact_contents(path: &str) -> String {
    let file = std::path::Path::new(path);
    let root = maestro_test_directory::temporary_root();
    assert_eq!(file.parent(), Some(root.as_path()));
    assert!(file.is_file());
    std::fs::read_to_string(file).unwrap()
}

#[test]
fn bash_reports_nonzero_command_status() {
    let tool = scripted(ScriptedShell::output("", Some(1.0)));
    let error = run_io((tool.execute)(
        "bash".into(),
        json!({"command":"exit 1"}),
        None,
        None,
    ))
    .unwrap_err();
    assert!(error.message.contains("code 1"), "{error:?}");
}

#[test]
fn bash_failure_artifacts_keep_complete_output() {
    if run_artifact_child("bash_failure_artifacts_keep_complete_output") {
        return;
    }
    let runtime = io_runtime();
    for (name, failure) in [
        ("timeout", ShellFailure::Timeout(5.0)),
        ("abort", ShellFailure::Aborted),
    ] {
        let chunks = (1..=3000)
            .map(|line| format!("{line}\n").into_bytes())
            .collect();
        let tool = scripted(ScriptedShell::raw(chunks, None, Some(failure)));
        let error = runtime
            .block_on((tool.execute)(
                "bash".into(),
                json!({"command":"chatty-fail"}),
                None,
                None,
            ))
            .unwrap_err();
        let artifact = error
            .message
            .split_once("Full output: ")
            .unwrap()
            .1
            .rsplit_once("]\n\n")
            .unwrap()
            .0;
        let observed = json!({"error":error.message,"artifact":artifact_contents(artifact)});
        assert_eq!(
            as_recorded(observed, &[(artifact, "$ARTIFACT")]),
            case(
                "tools_root",
                &format!("bash_failure_artifacts_keep_complete_output/{name}")
            )["expected"]
        );
    }
}

#[test]
fn bash_rejects_missing_working_directory() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    let absent = root.path().join("absent").display().to_string();
    let tool = create_bash_tool(
        absent.clone(),
        String::new(),
        BashToolOptions {
            shell_path: Some(root.path().display().to_string()),
            ..BashToolOptions::default()
        },
    );
    let error = run_io((tool.execute)(
        "bash".into(),
        json!({"command":"unused"}),
        None,
        None,
    ))
    .unwrap_err();
    assert!(
        error.message.contains("Working directory does not exist")
            && error.message.contains(&absent),
        "{error:?}"
    );
    assert!(!root.path().join("absent").exists());
}

#[test]
fn bash_preserves_shell_spawn_failure() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    let directory = root.path().display().to_string();
    assert!(root.path().is_dir());
    let tool = create_bash_tool(
        directory.clone(),
        String::new(),
        BashToolOptions {
            shell_path: Some(directory),
            ..BashToolOptions::default()
        },
    );
    let error = run_io((tool.execute)(
        "bash".into(),
        json!({"command":"unused"}),
        None,
        None,
    ))
    .unwrap_err();
    assert!(
        error.code.is_some(),
        "native spawn failures retain their code: {error:?}"
    );
    assert!(
        !error.message.contains("Custom shell path not found"),
        "{error:?}"
    );
}

#[test]
fn bash_routes_custom_shell_only_to_local_operations() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    run_io(async {
        let cwd = root.path().display().to_string();
        let absent = root.path().join("missing-shell").display().to_string();
        let tool = create_bash_tool(
            cwd.clone(),
            String::new(),
            BashToolOptions {
                operations: Some(Arc::new(ScriptedShell::output("", Some(0.0)))),
                shell_path: Some(absent.clone()),
                ..BashToolOptions::default()
            },
        );
        let result = (tool.execute)("bash".into(), json!({"command":"unused"}), None, None)
            .await
            .unwrap();
        assert_eq!(
            json!(result),
            case(
                "tools_root",
                "bash_routes_custom_shell_only_to_local_operations/supplied"
            )["expected"]["ok"]
        );
        let local = maestro_app::create_local_bash_operations(String::new(), Some(absent.clone()));
        let error = local
            .exec(
                "unused",
                &cwd,
                maestro_app::BashExecOptions {
                    on_data: &mut |_| panic!("missing shell must not produce output"),
                    signal: None,
                    timeout: None,
                    env: None,
                },
            )
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("Custom shell path not found")
                && error.to_string().contains(&absent),
            "{error:?}"
        );
    });
}

#[test]
fn bash_coalesces_chatty_progress() {
    if run_artifact_child("bash_coalesces_chatty_progress") {
        return;
    }
    let result = on_paused_task_set(|_| async {
        let chunks = (0..5000)
            .map(|line| format!("line {line}\n").into_bytes())
            .collect();
        let tool = scripted(ScriptedShell::raw(chunks, Some(0.0), None));
        let updates = Arc::new(Mutex::new(0_usize));
        let record = Arc::clone(&updates);
        let result = (tool.execute)(
            "bash".into(),
            json!({"command":"chatty"}),
            None,
            Some(Arc::new(move |_| *record.lock().unwrap() += 1)),
        )
        .await
        .unwrap();
        assert!(*updates.lock().unwrap() < 25);
        json!(result)
    });
    let artifact = result["details"]["fullOutputPath"].as_str().unwrap();
    let observed = json!({"ok":{"result":result},"artifact":artifact_contents(artifact)});
    assert_eq!(
        as_recorded(observed, &[(artifact, "$ARTIFACT")]),
        case("tools_root", "bash_coalesces_chatty_progress")["expected"]
    );
}

#[test]
fn bash_decodes_split_utf8_output() {
    let tool = scripted(ScriptedShell::raw(
        vec![vec![0xe2], vec![0x82, 0xac, b'\n']],
        Some(0.0),
        None,
    ));
    let result = run_io((tool.execute)(
        "bash".into(),
        json!({"command":"split-utf8"}),
        None,
        None,
    ))
    .unwrap();
    assert_eq!(
        json!(result),
        case("tools_root", "bash_decodes_split_utf8_output")["expected"]["ok"]
    );
}

#[test]
fn direct_shell_operation_cleans_terminal_output() {
    let operations = ScriptedShell::output("\x1b[31mred\x1b[0m\r\n", Some(0.0));
    let result = run_io(maestro_app::execute_bash_with_operations(
        "output",
        "/work",
        &operations,
        maestro_app::BashExecutorOptions::default(),
    ))
    .unwrap();
    assert_eq!(
        bash_result_view(&result),
        case(
            "tools_root",
            "direct_shell_operation_cleans_terminal_output"
        )["expected"]["ok"]
    );
}

#[test]
fn tool_shell_line_truncation_retains_artifact() {
    if run_artifact_child("tool_shell_line_truncation_retains_artifact") {
        return;
    }
    let mut content = String::new();
    for line in 1..=3000 {
        writeln!(content, "{line}").unwrap();
    }
    let tool = scripted(ScriptedShell::output(&content, Some(0.0)));
    let result = json!(
        run_io((tool.execute)(
            "bash".into(),
            json!({"command":"lines"}),
            None,
            None
        ))
        .unwrap()
    );
    let artifact = result["details"]["fullOutputPath"].as_str().unwrap();
    let observed = json!({"ok":result,"artifact":artifact_contents(artifact)});
    assert_eq!(
        as_recorded(observed, &[(artifact, "$ARTIFACT")]),
        case("tools_root", "tool_shell_line_truncation_retains_artifact")["expected"]
    );
}

#[test]
fn direct_shell_line_truncation_retains_artifact() {
    if run_artifact_child("direct_shell_line_truncation_retains_artifact") {
        return;
    }
    let mut content = String::new();
    for line in 1..=3000 {
        writeln!(content, "{line}").unwrap();
    }
    let shell = ScriptedShell::output(&content, Some(0.0));
    let result = run_io(maestro_app::execute_bash_with_operations(
        "lines",
        "/work",
        &shell,
        maestro_app::BashExecutorOptions::default(),
    ))
    .unwrap();
    let artifact = result.full_output_path.as_ref().unwrap();
    let observed = json!({"ok":bash_result_view(&result),"artifact":artifact_contents(artifact)});
    assert_eq!(
        as_recorded(observed, &[(artifact, "$ARTIFACT")]),
        case(
            "tools_root",
            "direct_shell_line_truncation_retains_artifact"
        )["expected"]
    );
}
