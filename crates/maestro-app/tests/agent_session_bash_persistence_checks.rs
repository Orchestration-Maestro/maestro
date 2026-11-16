//! Direct-shell timing and stored message order in complete application sessions.
#![cfg(not(target_arch = "wasm32"))]

use crate::support::session_harness::{
    Harness, bash_result_view, case, custom_note, echo_tools, entries_view, run_native_tool_child,
    start_prompt, transcribed, waiting_tool,
};
use maestro_app::test_support::{
    HarnessOptions, Scripted, answer, harness, on_task_set, recorded_number, script_responses,
};
use maestro_app::{AgentSessionEvent, agent_session::BashRunOptions};
use maestro_session::{
    message_records::SendMessageOptions, session_manager::ReadonlySessionManager,
};
use maestro_test_directory::Scratch;
use maestro_tools::BashResult;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// The live roles, stored entry types and complete shell messages without timestamps.
fn history(h: &Harness) -> Value {
    let messages = h.session.messages();
    let records = serde_json::to_value(&*messages.read().unwrap()).unwrap();
    let records = records.as_array().unwrap();
    let shell = records
        .iter()
        .filter(|message| message["role"] == "bashExecution")
        .map(|message| {
            let mut message = message.clone();
            message.as_object_mut().unwrap().shift_remove("timestamp");
            if let Some(code) = message["exitCode"].as_f64() {
                message["exitCode"] = recorded_number(code);
            }
            message
        })
        .collect::<Vec<_>>();
    let manager = h.session.session_manager().borrow();
    json!({
        "roles":records.iter().map(|message| &message["role"]).collect::<Vec<_>>(),
        "entries":manager.get_entries().iter().map(|entry| &entry["type"]).collect::<Vec<_>>(),
        "shell":shell,
    })
}

/// One point in the deferred shell's lifecycle.
fn phase(h: &Harness, name: &str) -> Value {
    let history = history(h);
    json!({"phase":name,"pending":h.session.has_pending_bash_messages(),"roles":history["roles"],"entries":history["entries"]})
}

/// A successful shell result recorded without starting a process.
fn finished() -> BashResult {
    BashResult {
        output: "hi".into(),
        exit_code: Some(0.0),
        cancelled: false,
        truncated: false,
        full_output_path: None,
    }
}

#[test]
fn idle_shell_records_persist_without_message_events() {
    on_task_set(|_| async {
        let (h, t) = transcribed(HarnessOptions::default()).await;
        h.session
            .record_bash_result("echo hi", &finished(), None)
            .unwrap();
        let mut observed = history(&h);
        observed["pending"] = json!(h.session.has_pending_bash_messages());
        observed["messageEndRoles"] = json!(
            t.events_of(&["message_end"])
                .iter()
                .map(|event| event["message"]["role"].clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            observed,
            case(
                "bash_persistence",
                "idle_shell_records_persist_without_message_events"
            )["expected"]
        );
    })
    .unwrap();
}

#[test]
fn streaming_shell_records_flush_when_run_settles() {
    on_task_set(|_| async {
        let started = Rc::new(tokio::sync::Notify::new());
        let release = Rc::new(tokio::sync::Notify::new());
        let tools = indexmap::IndexMap::from([(
            "wait".into(),
            waiting_tool(Rc::clone(&started), Rc::clone(&release)),
        )]);
        let h = harness(Scratch::new().unwrap(), HarnessOptions {
            base_tools_override: Some(tools),
            responses: Some(vec![
                Scripted {
                    tool_calls: vec![("wait".into(), json!({}))],
                    ..Scripted::default()
                },
                Scripted::text("done"),
                Scripted::text("after flush"),
            ]),
            ..HarnessOptions::default()
        })
        .await.unwrap();
        let running = start_prompt(&h, "start");
        started.notified().await;
        assert!(h.session.is_streaming());
        h.session
            .record_bash_result("echo hi", &finished(), None)
            .unwrap();
        let streaming = phase(&h, "streaming");
        release.notify_one();
        running.await.unwrap().unwrap();
        assert!(!h.session.is_streaming());
        let idle = phase(&h, "idle-before-next-prompt");
        let later = BashResult {
            output: "later".into(),
            ..finished()
        };
        h.session
            .record_bash_result("echo later", &later, None)
            .unwrap();
        start_prompt(&h, "next turn").await.unwrap().unwrap();
        let after = phase(&h, "after-next-prompt");
        let stored = entries_view(&h)
            .into_iter()
            .filter(|entry| entry["message"]["role"] == "bashExecution")
            .map(|entry| entry["message"]["command"].clone())
            .collect::<Vec<_>>();
        let observed = json!({"views":[streaming,idle,after],"shell":history(&h)["shell"],"storedShellCommands":stored});
        assert_eq!(
            observed,
            case(
                "bash_persistence",
                "streaming_shell_records_flush_when_run_settles"
            )["expected"]
        );
    }).unwrap();
}

/// A pending operation that settles only after its caller's cancellation signal fires.
struct CancelledShell(Rc<tokio::sync::Notify>);

impl maestro_app::BashOperations<maestro_models::Local> for CancelledShell {
    fn exec<'a>(
        &'a self,
        _: &'a str,
        _: &'a str,
        options: maestro_tools::BashExecOptions<'a, dyn FnMut(&[u8]) + 'a>,
    ) -> <maestro_models::Local as maestro_models::Execution>::Future<
        'a,
        Result<maestro_tools::BashExecResult, maestro_tools::BashError>,
    > {
        Box::pin(async move {
            self.0.notify_one();
            options.signal.unwrap().cancelled().await;
            Err(maestro_tools::BashError::Aborted)
        })
    }
}

#[test]
fn abort_shell_clears_running_state_and_persists_cancellation() {
    on_task_set(|_| async {
        let h = harness(Scratch::new().unwrap(), HarnessOptions::default())
            .await
            .unwrap();
        let started = Rc::new(tokio::sync::Notify::new());
        let run = h.session.execute_bash(
            "sleep",
            None,
            BashRunOptions {
                operations: Some(Rc::new(CancelledShell(Rc::clone(&started)))),
                exclude_from_context: None,
            },
        );
        started.notified().await;
        let before = h.session.is_bash_running();
        h.session.abort_bash();
        let result = run.await.unwrap();
        let mut observed = history(&h);
        observed["before"] = json!(before);
        observed["after"] = json!(h.session.is_bash_running());
        observed["result"] = bash_result_view(&result);
        assert_eq!(
            observed,
            case(
                "bash_persistence",
                "abort_shell_clears_running_state_and_persists_cancellation"
            )["expected"]
        );
    })
    .unwrap();
}

#[test]
fn custom_user_assistant_and_tool_messages_keep_record_order() {
    on_task_set(|_| async {
        let tools = echo_tools(Rc::new(RefCell::new(false)));
        tools["echo"].write().unwrap().execute = Rc::new(|_, arguments, _, _| {
            let text = arguments["text"].as_str().unwrap().to_owned();
            Box::pin(std::future::ready(Ok(
                maestro_agent::AgentToolResult::text_result(
                    format!("echo:{text}"),
                    Some(json!({"text":text})),
                ),
            )))
        });
        let h = harness(
            Scratch::new().unwrap(),
            HarnessOptions {
                base_tools_override: Some(tools),
                responses: Some(vec![Scripted::echo_call("hello"), Scripted::text("done")]),
                ..HarnessOptions::default()
            },
        )
        .await
        .unwrap();
        let mut note = custom_note("hello");
        note.details = maestro_session::Presence::Present(json!({"a":1}));
        h.session
            .send_custom_message(
                note,
                SendMessageOptions {
                    trigger_turn: Some(false),
                    deliver_as: None,
                },
            )
            .await
            .unwrap();
        start_prompt(&h, "start").await.unwrap().unwrap();
        let records = {
            let messages = h.session.messages();
            serde_json::to_value(&*messages.read().unwrap()).unwrap()
        };
        let results = records
            .as_array()
            .unwrap()
            .iter()
            .filter(|record| record["role"] == "toolResult")
            .map(|record| json!({"content":record["content"],"details":record["details"]}))
            .collect::<Vec<_>>();
        let mut observed = history(&h);
        observed.as_object_mut().unwrap().shift_remove("shell");
        observed["toolResults"] = json!(results);
        assert_eq!(
            observed,
            case(
                "bash_persistence",
                "custom_user_assistant_and_tool_messages_keep_record_order"
            )["expected"]
        );
    })
    .unwrap();
}

#[test]
fn interrupted_assistant_is_kept_in_session_entries() {
    on_task_set(|_| async {
        let h = harness(Scratch::new().unwrap(), HarnessOptions::default()).await.unwrap();
        script_responses(&h, vec![answer(&"x".repeat(20_000), None, None)]);
        let updated = Rc::new(tokio::sync::Notify::new());
        let notify = Rc::clone(&updated);
        drop(h.session.subscribe(Rc::new(move |event| {
            if let AgentSessionEvent::Agent(event) = event
                && matches!(
                    *event.read().unwrap(),
                    maestro_agent::AgentEvent::MessageUpdate { .. }
                )
            {
                notify.notify_one();
            }
        })));
        let running = start_prompt(&h, "hi");
        updated.notified().await;
        h.session.abort().await;
        running.await.unwrap().unwrap();
        let manager = h.session.session_manager().borrow();
        let entries = manager.get_entries();
        let last = entries.last().unwrap();
        let mut observed = history(&h);
        observed.as_object_mut().unwrap().shift_remove("shell");
        observed["last"] = json!({"type":last["type"],"role":last["message"]["role"],"stopReason":last["message"]["stopReason"]});
        assert_eq!(
            observed,
            case(
                "bash_persistence",
                "interrupted_assistant_is_kept_in_session_entries"
            )["expected"]
        );
    }).unwrap();
}

#[test]
fn direct_shell_runs_and_records_native_output() {
    if run_native_tool_child("direct_shell_runs_and_records_native_output") {
        return;
    }
    on_task_set(|_| async {
        let h = harness(Scratch::new().unwrap(), HarnessOptions::default())
            .await
            .unwrap();
        let cwd = h.session.session_manager().borrow().get_cwd().to_owned();
        std::fs::create_dir_all(cwd).unwrap();
        let result = h
            .session
            .execute_bash("printf 'hello'", None, BashRunOptions::default())
            .await
            .unwrap();
        let mut observed = history(&h);
        observed["result"] = bash_result_view(&result);
        assert_eq!(
            observed,
            case(
                "bash_persistence",
                "direct_shell_runs_and_records_native_output"
            )["expected"]
        );
    })
    .unwrap();
}
