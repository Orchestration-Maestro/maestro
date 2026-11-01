//! Direct shell commands: the configured prefix, directory and shell path, progress,
//! cancellation and the recorded transcript, compared with the recorded shell cases.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    Harness, HarnessOptions, ScriptedShell, Transcript, assistant, bash_result_view, case,
    entries_view, harness, hold_first_answer, labelled, messages_view, on_task_set, queue_view,
    start_prompt, supplied_failure, transcribed, with_placeholders,
};
use maestro_app::agent_session::{BashRunOptions, PromptOptions};
use maestro_session::SessionManager;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

/// The recorded shell case `name`.
fn expected(name: &str) -> Value {
    case("bash", name)
}

/// Runs `command` through `operations` on a session over `options`, returning the session,
/// its transcript and the result's recorded form.
async fn run(
    options: HarnessOptions,
    command: &str,
    operations: &Rc<ScriptedShell>,
    exclude_from_context: Option<bool>,
) -> (Harness, Transcript, Value) {
    let (h, t) = transcribed(options).await;
    let run_options = BashRunOptions {
        exclude_from_context,
        operations: Some(operations.clone()),
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
        let operations = Rc::new(ScriptedShell::output("prefixed output", Some(7.0)));
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
        let operations = Rc::new(ScriptedShell::output("", Some(0.0)));
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
        let operations = Rc::new(ScriptedShell::chunks(
            vec!["one ".into(), "two".into()],
            Some(0.0),
        ));
        let chunks = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&chunks);
        let collect = Box::new(move |chunk: &str| sink.lock().unwrap().push(chunk.to_owned()));
        let options = BashRunOptions {
            exclude_from_context: None,
            operations: Some(operations.clone()),
        };
        let result = h
            .session
            .execute_bash("stream", Some(collect), options)
            .await;
        let result = json!({"ok": bash_result_view(&result.unwrap())});
        assert_eq!(
            json!([result, labelled("chunks", &*chunks.lock().unwrap())]),
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
        let operations = Rc::new(ScriptedShell::held(vec!["partial".into()], None));
        let options = BashRunOptions {
            exclude_from_context: None,
            operations: Some(operations.clone()),
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
        let operations = Rc::new(ScriptedShell::held(Vec::new(), Some(0.0)));
        let options = BashRunOptions {
            exclude_from_context: None,
            operations: Some(operations.clone()),
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
        let operations = Rc::new(ScriptedShell::output("", Some(0.0)));
        let (h, t, result) = run(HarnessOptions::default(), "echo quiet", &operations, None).await;
        assert_eq!(json!([result]), expected["results"]);
        assert_shell_case(&h, &t, &expected);
    });
}

#[test]
fn excluded_shell_result_is_recorded_without_context() {
    on_task_set(|_| async {
        let expected = expected("an excluded shell result is recorded without context");
        let operations = Rc::new(ScriptedShell::output("", Some(0.0)));
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
        let operations = Rc::new(ScriptedShell::output(&"x".repeat(60 * 1024), Some(0.0)));
        let (h, t, result) = run(HarnessOptions::default(), "big", &operations, None).await;
        assert_eq!(json!([result]), expected["results"]);
        assert_shell_case(&h, &t, &expected);
        let messages = h.session.messages();
        let record = serde_json::to_value(&messages.read().unwrap()[0]).unwrap();
        let artifact = record["fullOutputPath"].as_str().unwrap();
        assert_eq!(std::fs::read(artifact).unwrap().len(), 60 * 1024);
    });
}

/// Shell operations that fail with `0`.
struct FailingShell(maestro_models::DiagnosticErrorInfo);

impl maestro_tools::BashOperations<maestro_models::Local> for FailingShell {
    fn exec<'a>(
        &'a self,
        _: &'a str,
        _: &'a str,
        _: maestro_tools::BashExecOptions<'a, dyn FnMut(&[u8]) + 'a>,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<maestro_tools::BashExecResult, maestro_tools::BashError>,
                > + 'a,
        >,
    > {
        let failure = maestro_tools::BashError::Other(Box::new(self.0.clone()));
        Box::pin(std::future::ready(Err(failure)))
    }
}

#[test]
fn shell_failure_keeps_its_supplied_fields() {
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let options = BashRunOptions {
            exclude_from_context: None,
            operations: Some(Rc::new(FailingShell(supplied_failure()))),
        };
        let failure = h.session.execute_bash("remote", None, options).await;
        assert_eq!(failure.unwrap_err(), supplied_failure());
        assert!(!h.session.is_bash_running());
    });
}

#[test]
fn shell_starts_and_accepts_abort_before_observation() {
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let operations = Rc::new(ScriptedShell::held(vec!["chunk".into()], Some(7.0)));
        let observer = h.session.execute_bash(
            "shell",
            None,
            BashRunOptions {
                exclude_from_context: None,
                operations: Some(operations.clone()),
            },
        );
        let running_at_call = h.session.is_bash_running();
        let executed_at_call = operations.executed.lock().unwrap().len();
        h.session.abort_bash();
        operations.release.notify_one();
        let result = observer.await.unwrap();
        assert_eq!(
            (running_at_call, executed_at_call, result.cancelled),
            (true, 1, true)
        );
        assert!(!h.session.is_bash_running());
    });
}

#[test]
fn abandoned_shell_command_still_settles_and_records() {
    on_task_set(|_| async {
        let tasks = Rc::new(RefCell::new(Vec::new()));
        let queue = Rc::clone(&tasks);
        let h = harness(HarnessOptions {
            spawn: Some(Rc::new(move |task| queue.borrow_mut().push(task))),
            ..HarnessOptions::default()
        })
        .await;
        let operations = Rc::new(ScriptedShell::held(vec!["late".into()], Some(0.0)));
        let options = BashRunOptions {
            exclude_from_context: None,
            operations: Some(operations.clone()),
        };
        drop(h.session.execute_bash("slow", None, options));
        assert!(h.session.is_bash_running());
        let task = tasks.borrow_mut().pop();
        let task = task.expect("the session's facility holds the command");
        operations.release.notify_one();
        task.await;
        assert!(!h.session.is_bash_running());
        assert_eq!(shell_commands(&h), [json!("slow")]);
    });
}

/// The commands of the shell results in the history, in order.
fn shell_commands(h: &Harness) -> Vec<Value> {
    messages_view(h)
        .iter()
        .filter(|message| message["role"] == "bashExecution")
        .map(|message| message["command"].clone())
        .collect()
}

#[test]
fn failed_flush_keeps_the_unwritten_shell_results() {
    on_task_set(|_| async {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let root = scratch.path().display().to_string();
        let sessions = format!("{root}/sessions");
        let manager =
            SessionManager::create(&format!("{root}/project"), Some(&sessions), &root).unwrap();
        let h = harness(HarnessOptions {
            session_manager: Some(manager),
            scratch: Some(scratch),
            ..HarnessOptions::default()
        })
        .await;
        let seeded = assistant("seeded", 1, &json!({}));
        drop(
            h.session
                .session_manager()
                .borrow_mut()
                .append_message(seeded)
                .unwrap(),
        );
        let (requested, release) = hold_first_answer(&h, "first", &["third"]);
        let running = start_prompt(&h, "first");
        requested.notified().await;
        let deferred = |command| {
            let finished = maestro_tools::BashResult {
                output: String::new(),
                exit_code: Some(0.0),
                cancelled: false,
                truncated: false,
                full_output_path: None,
            };
            h.session
                .record_bash_result(command, &finished, None)
                .unwrap();
        };
        deferred("one");
        deferred("two");
        std::fs::remove_dir_all(&sessions).unwrap();
        release.notify_one();
        let failed = running.await.unwrap();
        assert!(failed.is_err(), "the settle step reports the failed flush");
        assert!(h.session.has_pending_bash_messages());
        assert_eq!(shell_commands(&h), [json!("one")]);
        std::fs::create_dir_all(&sessions).unwrap();
        h.session
            .prompt("third", PromptOptions::default())
            .await
            .unwrap();
        assert!(!h.session.has_pending_bash_messages());
        assert_eq!(shell_commands(&h), [json!("one"), json!("two")]);
    });
}

/// A caller-local operation checks synchronous output delivery before it settles.
struct LocalShell(Rc<RefCell<Vec<String>>>);

impl maestro_tools::BashOperations<maestro_models::Local> for LocalShell {
    fn exec<'a>(
        &'a self,
        command: &'a str,
        cwd: &'a str,
        options: maestro_tools::BashExecOptions<'a, dyn FnMut(&[u8]) + 'a>,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<maestro_tools::BashExecResult, maestro_tools::BashError>,
                > + 'a,
        >,
    > {
        Box::pin(async move {
            assert_eq!(command, "prefix\nlocal");
            assert!(!cwd.is_empty());
            assert!(options.signal.is_some());
            assert!(options.timeout.is_none());
            assert!(options.env.is_none());
            (options.on_data)(b"\x1b[31mone\x1b[0m\r\n");
            assert_eq!(*self.0.borrow(), ["one\n"]);
            (options.on_data)("é".as_bytes());
            assert_eq!(*self.0.borrow(), ["one\n", "é"]);
            Ok(maestro_tools::BashExecResult {
                exit_code: Some(7.0),
            })
        })
    }
}

#[test]
fn shell_accepts_local_operations_and_chunk_observers() {
    on_task_set(|_| async {
        let (h, _) = transcribed(with_settings(json!({"shellCommandPrefix":"prefix"}))).await;
        let chunks = Rc::new(RefCell::new(Vec::new()));
        let observed = Rc::clone(&chunks);
        let result = h
            .session
            .execute_bash(
                "local",
                Some(Box::new(move |chunk| {
                    observed.borrow_mut().push(chunk.to_owned());
                })),
                BashRunOptions {
                    operations: Some(Rc::new(LocalShell(Rc::clone(&chunks)))),
                    exclude_from_context: Some(true),
                },
            )
            .await
            .unwrap();
        assert_eq!(result.output, "one\né");
        assert_eq!(result.exit_code, Some(7.0));
        assert!(!result.cancelled);
        assert!(!result.truncated);
        assert!(result.full_output_path.is_none());
        let records = messages_view(&h);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["command"], "local");
        assert_eq!(records[0]["output"], "one\né");
        assert_eq!(records[0]["excludeFromContext"], true);
        assert!(!h.session.is_bash_running());
    });
}
