//! Direct shell commands: the configured prefix, directory and shell path, progress,
//! cancellation and the recorded transcript, compared with the recorded shell cases.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    Harness, HarnessOptions, ScriptedShell, Transcript, bash_result_view, case, entries_view,
    labelled, messages_view, on_task_set, queue_view, transcribed, with_placeholders,
};
use maestro_app::agent_session::BashRunOptions;
use serde_json::{Value, json};

/// The recorded shell case `name`.
fn expected(name: &str) -> Value {
    case("bash", name)
}

/// Runs `command` through `operations` on a session over `options`, returning the session,
/// its transcript and the result's recorded form.
async fn run(
    options: HarnessOptions,
    command: &str,
    operations: &ScriptedShell,
    exclude_from_context: Option<bool>,
) -> (Harness, Transcript, Value) {
    let (h, t) = transcribed(options).await;
    let run_options = BashRunOptions {
        exclude_from_context,
        operations: Some(operations),
    };
    let result = h
        .session
        .execute_bash(command, None, run_options)
        .await
        .unwrap();
    (h, t, json!({"ok": bash_result_view(&result)}))
}

/// Asserts the recorded history, stored entries, announcements and queue of `case`: the
/// result is stored at once and announces no message.
fn assert_shell_case(h: &Harness, t: &Transcript, case: &Value) {
    assert_eq!(
        with_placeholders(json!(messages_view(h)), h),
        case["messages"]
    );
    assert_eq!(
        with_placeholders(json!(entries_view(h)), h),
        case["entries"]
    );
    assert_eq!(json!(*t.events.borrow()), case["sessionEvents"]);
    assert_eq!(queue_view(h, t), case["queue"]);
    assert!(!h.session.has_pending_bash_messages());
}

/// Options with `settings` as the saved preferences.
fn with_settings(settings: Value) -> HarnessOptions {
    HarnessOptions {
        settings,
        ..HarnessOptions::default()
    }
}

#[test]
fn shell_prefix_applies_to_execution_only() {
    on_task_set(|_| async {
        let expected =
            expected("a direct shell records the original command and runs the prefixed one");
        let operations = ScriptedShell::output("prefixed output", Some(7.0));
        let settings = with_settings(json!({"shellCommandPrefix": "shopt -s expand_aliases"}));
        let (h, t, result) = run(settings, "echo hi", &operations, None).await;
        let executed = with_placeholders(json!(*operations.executed.lock().unwrap()), &h);
        assert_eq!(
            json!([result, labelled("executed", executed)]),
            expected["results"]
        );
        assert_shell_case(&h, &t, &expected);
    });
}

#[test]
fn shell_runs_in_the_session_directory() {
    on_task_set(|_| async {
        let expected = expected("a direct shell uses the session directory");
        let operations = ScriptedShell::output("", Some(0.0));
        let (h, t, result) = run(HarnessOptions::default(), "pwd", &operations, None).await;
        let executed = with_placeholders(json!(*operations.executed.lock().unwrap()), &h);
        assert_eq!(
            json!([result, labelled("executedCwd", executed)]),
            expected["results"]
        );
        assert_shell_case(&h, &t, &expected);
    });
}

#[test]
fn shell_progress_reaches_the_caller() {
    on_task_set(|_| async {
        let expected = expected("progress chunks reach the caller");
        let (h, t) = transcribed(HarnessOptions::default()).await;
        let operations = ScriptedShell::chunks(vec!["one ".into(), "two".into()], Some(0.0));
        let mut chunks = Vec::new();
        let mut collect = |chunk: &str| chunks.push(chunk.to_owned());
        let options = BashRunOptions {
            exclude_from_context: None,
            operations: Some(&operations),
        };
        let result = h
            .session
            .execute_bash("stream", Some(&mut collect), options)
            .await;
        let result = json!({"ok": bash_result_view(&result.unwrap())});
        assert_eq!(
            json!([result, labelled("chunks", chunks)]),
            expected["results"]
        );
        assert_shell_case(&h, &t, &expected);
    });
}

#[test]
fn cancelled_shell_is_recorded_as_cancelled() {
    on_task_set(|_| async {
        let expected = expected("a cancelled command is recorded as cancelled");
        let (h, t) = transcribed(HarnessOptions::default()).await;
        let operations = ScriptedShell::held(vec!["partial".into()], None);
        let options = BashRunOptions {
            exclude_from_context: None,
            operations: Some(&operations),
        };
        let cancel = async {
            operations.started.notified().await;
            h.session.abort_bash();
            operations.release.notify_one();
        };
        let (result, ()) = tokio::join!(h.session.execute_bash("sleep 1", None, options), cancel);
        let result = json!({"ok": bash_result_view(&result.unwrap())});
        assert_eq!(json!([result]), expected["results"]);
        assert_shell_case(&h, &t, &expected);
    });
}

#[test]
fn running_shell_state_spans_the_command() {
    on_task_set(|_| async {
        let expected = expected("a running command reports its state until it settles");
        let (h, t) = transcribed(HarnessOptions::default()).await;
        let operations = ScriptedShell::held(Vec::new(), Some(0.0));
        let options = BashRunOptions {
            exclude_from_context: None,
            operations: Some(&operations),
        };
        assert!(!h.session.is_bash_running());
        let observe = async {
            operations.started.notified().await;
            let during = (
                h.session.is_bash_running(),
                h.session.has_pending_bash_messages(),
            );
            operations.release.notify_one();
            during
        };
        let (result, (running, pending)) =
            tokio::join!(h.session.execute_bash("slow", None, options), observe);
        let results = json!([
            {"ok": bash_result_view(&result.unwrap())},
            labelled("runningDuring", running),
            labelled("runningAfter", h.session.is_bash_running()),
            labelled("pendingDuring", pending),
        ]);
        assert_eq!(results, expected["results"]);
        assert_shell_case(&h, &t, &expected);
    });
}

#[cfg(unix)]
#[test]
fn configured_shell_path_runs_the_command() {
    on_task_set(|_| async {
        let expected = expected("a configured shell path runs the command");
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let project = scratch.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        let source = scratch.path().join("marker-source.sh");
        std::fs::write(&source, "#!/bin/sh\necho marker-shell\n").unwrap();
        let shell = project.join("marker.sh");
        crate::native::install_executable(&source, &shell);
        let (h, t) = transcribed(HarnessOptions {
            settings: json!({"shellPath": shell.display().to_string()}),
            scratch: Some(scratch),
            ..HarnessOptions::default()
        })
        .await;
        let result = h
            .session
            .execute_bash("ignored", None, BashRunOptions::default())
            .await;
        let result = json!({"ok": bash_result_view(&result.unwrap())});
        assert_eq!(json!([result]), expected["results"]);
        assert_shell_case(&h, &t, &expected);
    });
}

#[test]
fn shell_records_announce_no_messages() {
    on_task_set(|_| async {
        let expected = expected("a shell result emits no message events");
        let operations = ScriptedShell::output("", Some(0.0));
        let (h, t, result) = run(HarnessOptions::default(), "echo quiet", &operations, None).await;
        assert_eq!(json!([result]), expected["results"]);
        assert_shell_case(&h, &t, &expected);
    });
}

#[test]
fn excluded_shell_result_is_recorded_without_context() {
    on_task_set(|_| async {
        let expected = expected("an excluded shell result is recorded without context");
        let operations = ScriptedShell::output("", Some(0.0));
        let (h, t, result) = run(
            HarnessOptions::default(),
            "echo secret",
            &operations,
            Some(true),
        )
        .await;
        assert_eq!(json!([result]), expected["results"]);
        assert_shell_case(&h, &t, &expected);
    });
}

#[test]
fn truncated_shell_result_keeps_its_artifact() {
    on_task_set(|_| async {
        let expected = expected("a truncated result keeps its overflow path");
        let operations = ScriptedShell::output(&"x".repeat(60 * 1024), Some(0.0));
        let (h, t, result) = run(HarnessOptions::default(), "big", &operations, None).await;
        assert_eq!(json!([result]), expected["results"]);
        assert_shell_case(&h, &t, &expected);
        let messages = h.session.messages();
        let record = serde_json::to_value(&messages.read().unwrap()[0]).unwrap();
        let artifact = record["fullOutputPath"].as_str().unwrap();
        assert_eq!(std::fs::read(artifact).unwrap().len(), 60 * 1024);
    });
}
