//! The session over the scripted stream: responses in sequence and wrapping around, tool
//! execution, failures and retries, usage, delivered events and streamed deltas, provider
//! contexts, inline extensions and persistence, compared with the recorded harness cases.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    Harness, HarnessOptions, Scripted, corpus, delivered_events, echo_tools, harness,
    on_paused_task_set, on_task_set, recorded_number, retry_order,
};
use maestro_extensions::{ExtensionFactory, types::RegisteredCommandOptions};
use maestro_models::Local;
use maestro_session::session_manager::ReadonlySessionManager;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// The recorded harness case `name`.
fn expected(name: &str) -> Value {
    corpus("harness")[name].clone()
}

/// A session over the scripted stream answering with `responses`.
async fn scripted(responses: Vec<Scripted>) -> Harness {
    harness(HarnessOptions {
        responses: Some(responses),
        ..HarnessOptions::default()
    })
    .await
}

/// Sends `text` and waits for the session to settle.
async fn prompt(h: &Harness, text: &str) {
    h.session
        .prompt(text, maestro_app::agent_session::PromptOptions::default())
        .await
        .unwrap();
}

/// The agent history as records.
fn records(h: &Harness) -> Vec<Value> {
    let messages = h.session.messages();
    let messages = messages.read().unwrap();
    messages
        .iter()
        .map(|message| serde_json::to_value(message).unwrap())
        .collect()
}

/// The records of `role`.
fn of_role(h: &Harness, role: &str) -> Vec<Value> {
    records(h)
        .into_iter()
        .filter(|record| record["role"] == role)
        .collect()
}

/// The text of each assistant message's first text block.
fn assistant_texts(h: &Harness) -> Vec<Value> {
    of_role(h, "assistant")
        .iter()
        .map(|record| {
            let blocks = record["content"].as_array().unwrap();
            let text = blocks.iter().find(|block| block["type"] == "text");
            text.map_or(Value::Null, |block| block["text"].clone())
        })
        .collect()
}

/// The roles of the agent history.
fn roles(h: &Harness) -> Value {
    json!(
        records(h)
            .iter()
            .map(|record| record["role"].clone())
            .collect::<Vec<_>>()
    )
}

/// Event types with each run of stream updates as one, as the recorded counts vary.
fn collapsed(types: impl IntoIterator<Item = Value>) -> Vec<Value> {
    let mut types: Vec<Value> = types.into_iter().collect();
    types.dedup_by(|next, previous| next == "message_update" && previous == "message_update");
    types
}

/// The delivered events of `kind`.
fn of_type(events: &[Value], kind: &str) -> Vec<Value> {
    events
        .iter()
        .filter(|event| event["type"] == kind)
        .cloned()
        .collect()
}

/// The stream updates of `update` kind.
fn updates(events: &[Value], update: &str) -> Vec<Value> {
    events
        .iter()
        .filter(|event| event["update"] == update)
        .cloned()
        .collect()
}

/// The concatenated deltas of `events`.
fn joined(events: &[Value]) -> String {
    events
        .iter()
        .filter_map(|event| event["delta"].as_str())
        .collect()
}

/// A session with the echo tool in place of the built-in tools, answering with `responses`.
async fn echo_session(responses: Vec<Scripted>, executed: Rc<RefCell<bool>>) -> Harness {
    harness(HarnessOptions {
        responses: Some(responses),
        base_tools_override: Some(echo_tools(executed)),
        ..HarnessOptions::default()
    })
    .await
}

#[test]
fn observes_simple_text_response() {
    let expected = expected("simple_text_response");
    on_task_set(|_| async {
        let h = scripted(vec![Scripted::text("hello world")]).await;
        let events = delivered_events(&h.session);
        prompt(&h, "hi").await;
        assert_eq!(json!(h.requests.0.borrow().len()), expected["call_count"]);
        let answers = of_role(&h, "assistant");
        assert_eq!(json!(answers.len()), expected["assistant_count"]);
        assert_eq!(answers[0]["content"], expected["content"]);
        assert_eq!(answers[0]["stopReason"], expected["stop_reason"]);
        assert_eq!(roles(&h), expected["message_roles"]);
        let types = events
            .borrow()
            .iter()
            .map(|event| event["type"].clone())
            .collect::<Vec<_>>();
        let recorded = expected["event_types"].as_array().unwrap().clone();
        assert_eq!(collapsed(types), collapsed(recorded));
    });
}

#[test]
fn observes_response_sequence() {
    let expected = expected("response_sequence");
    on_task_set(|_| async {
        let h = scripted(["first", "second", "third"].map(Scripted::text).to_vec()).await;
        for text in ["a", "b", "c"] {
            prompt(&h, text).await;
        }
        assert_eq!(json!(h.requests.0.borrow().len()), expected["call_count"]);
        assert_eq!(json!(assistant_texts(&h)), expected["assistant_texts"]);
    });
}

#[test]
fn observes_tool_call_response_triggers_tool_execution() {
    let expected = expected("tool_call_triggers_execution");
    on_task_set(|_| async {
        let executed = Rc::new(RefCell::new(false));
        let responses = vec![Scripted::echo_call("hi"), Scripted::text("done after tool")];
        let h = echo_session(responses, Rc::clone(&executed)).await;
        prompt(&h, "use the tool").await;
        assert_eq!(json!(*executed.borrow()), expected["tool_executed"]);
        assert_eq!(json!(h.requests.0.borrow().len()), expected["call_count"]);
        assert_eq!(
            json!(of_role(&h, "toolResult").len()),
            expected["tool_result_count"]
        );
        assert_eq!(roles(&h), expected["message_roles"]);
    });
}

#[test]
fn observes_error_response() {
    let expected = expected("error_response");
    on_task_set(|_| async {
        let failure = Scripted {
            error: Some("something broke".into()),
            ..Scripted::default()
        };
        let h = scripted(vec![failure]).await;
        prompt(&h, "hi").await;
        let answers = of_role(&h, "assistant");
        assert_eq!(json!(answers.len()), expected["assistant_count"]);
        assert_eq!(answers[0]["stopReason"], expected["stop_reason"]);
        assert_eq!(answers[0]["errorMessage"], expected["error_message"]);
    });
}

#[test]
fn observes_retry_on_transient_error() {
    let expected = expected("retry_on_transient_error");
    on_paused_task_set(|_| async {
        for answer in ["Success", "recovered"] {
            let h = harness(HarnessOptions {
                responses: Some(vec![
                    Scripted::failure("overloaded_error"),
                    Scripted::text(answer),
                ]),
                settings: json!({"retry": {"enabled": true, "maxRetries": 3, "baseDelayMs": 1}}),
                ..HarnessOptions::default()
            })
            .await;
            let events = delivered_events(&h.session);
            prompt(&h, "hi").await;
            let events = events.borrow();
            assert_eq!(
                retry_order(&events),
                [
                    format!("start:{}", expected["retry_start_attempt"]),
                    format!("end:{}", expected["retry_end_success"]),
                ]
            );
            let starts = of_type(&events, "auto_retry_start");
            assert_eq!(json!(h.requests.0.borrow().len()), expected["call_count"]);
            assert_eq!(starts[0]["delayMs"], expected["retry_start_delay_ms"]);
            assert_eq!(json!(assistant_texts(&h)), json!([answer]));
            assert!(!h.session.is_retrying(), "no retry is left in flight");
            assert!(!h.session.is_streaming(), "the response settled");
        }
    });
}

#[test]
fn observes_custom_usage_numbers() {
    let expected = expected("custom_usage_numbers");
    on_task_set(|_| async {
        let answer = Scripted {
            usage: Some((100_000.0, 5_000.0)),
            ..Scripted::text("big response")
        };
        let h = scripted(vec![answer]).await;
        prompt(&h, "hi").await;
        let usage = &of_role(&h, "assistant")[0]["usage"];
        let number = |key: &str| recorded_number(usage[key].as_f64().unwrap());
        assert_eq!(number("input"), expected["usage_input"]);
        assert_eq!(number("output"), expected["usage_output"]);
        assert_eq!(number("totalTokens"), expected["usage_total"]);
    });
}

#[test]
fn captures_session_events_in_order() {
    let expected = expected("event_capture");
    on_task_set(|_| async {
        let h = scripted(vec![Scripted::text("hello")]).await;
        let events = delivered_events(&h.session);
        prompt(&h, "hi").await;
        let events = events.borrow();
        assert_eq!(
            json!(of_type(&events, "agent_start").len()),
            expected["agent_start_count"]
        );
        assert_eq!(
            json!(of_type(&events, "agent_end").len()),
            expected["agent_end_count"]
        );
        assert_eq!(
            json!(of_type(&events, "message_end").len()),
            expected["message_end_count"]
        );
        let types = events.iter().map(|event| event["type"].clone());
        let recorded = expected["event_types"].as_array().unwrap().clone();
        assert_eq!(collapsed(types), collapsed(recorded));
    });
}

#[test]
fn captures_provider_context() {
    let expected = expected("context_capture");
    on_task_set(|_| async {
        let h = scripted(vec![Scripted::text("reply")]).await;
        prompt(&h, "my question").await;
        let requests = h.requests.0.borrow();
        assert_eq!(json!(requests.len()), expected["context_count"]);
        let context = serde_json::to_value(&requests[0]).unwrap();
        let roles: Vec<Value> = context["messages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|message| message["role"].clone())
            .collect();
        assert_eq!(json!(roles), expected["message_roles"]);
        assert_eq!(context["messages"][0]["content"], expected["user_text"]);
        let tools: Vec<Value> = context["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].clone())
            .collect();
        assert_eq!(json!(tools), expected["tool_names"]);
        assert_eq!(context["systemPrompt"], json!(h.session.system_prompt()));
    });
}

#[test]
fn cycles_scripted_responses() {
    let expected = expected("wraps_around");
    on_task_set(|_| async {
        let h = scripted(vec![Scripted::text("a"), Scripted::text("b")]).await;
        for text in ["1", "2", "3"] {
            prompt(&h, text).await;
        }
        assert_eq!(json!(h.requests.0.borrow().len()), expected["call_count"]);
        assert_eq!(json!(assistant_texts(&h)), expected["texts"]);
    });
}

#[test]
fn streams_text_delta_content() {
    let expected = expected("streams_text_deltas");
    on_task_set(|_| async {
        let h = scripted(vec![Scripted::text("hello world")]).await;
        let events = delivered_events(&h.session);
        prompt(&h, "hi").await;
        let deltas = updates(&events.borrow(), "text_delta");
        assert!(!deltas.is_empty(), "the text arrives in deltas");
        assert_eq!(json!(joined(&deltas)), expected["reconstructed"]);
    });
}

#[test]
fn streams_thinking_delta_content() {
    let expected = expected("streams_thinking_deltas");
    on_task_set(|_| async {
        let answer = Scripted {
            thinking: Some("let me think about this".into()),
            ..Scripted::text("answer")
        };
        let h = scripted(vec![answer]).await;
        let events = delivered_events(&h.session);
        prompt(&h, "hi").await;
        let events = events.borrow();
        let deltas = updates(&events, "thinking_delta");
        assert_eq!(
            json!(updates(&events, "thinking_start").len()),
            expected["thinking_start_count"]
        );
        assert_eq!(
            json!(updates(&events, "thinking_end").len()),
            expected["thinking_end_count"]
        );
        assert!(!deltas.is_empty(), "the thinking arrives in deltas");
        assert_eq!(json!(joined(&deltas)), expected["reconstructed"]);
    });
}

#[test]
fn tool_call_deltas_stream_before_the_call_runs() {
    let expected = expected("streams_tool_call_deltas");
    on_task_set(|_| async {
        let executed = Rc::new(RefCell::new(false));
        let h = echo_session(
            vec![Scripted::echo_call("hi"), Scripted::text("done")],
            executed,
        )
        .await;
        let events = delivered_events(&h.session);
        prompt(&h, "use tool").await;
        let events = events.borrow();
        assert_eq!(
            json!(updates(&events, "toolcall_start").len()),
            expected["toolcall_start_count"]
        );
        assert_eq!(
            json!(updates(&events, "toolcall_end").len()),
            expected["toolcall_end_count"]
        );
        assert!(
            !updates(&events, "toolcall_delta").is_empty(),
            "the arguments arrive in deltas"
        );
    });
}

#[test]
fn orders_thinking_text_and_tool_call_streams() {
    on_task_set(|_| async {
        let executed = Rc::new(RefCell::new(false));
        let answer = Scripted {
            thinking: Some("hmm".into()),
            ..Scripted {
                text: Some("I will call a tool".into()),
                ..Scripted::echo_call("x")
            }
        };
        let h = echo_session(vec![answer, Scripted::text("final")], executed).await;
        let events = delivered_events(&h.session);
        prompt(&h, "do it").await;
        let kinds: Vec<Value> = events
            .borrow()
            .iter()
            .filter(|event| event["type"] == "message_update")
            .map(|event| event["update"].clone())
            .collect();
        let first = |kind: &str| kinds.iter().position(|update| update == kind).unwrap();
        assert!(first("thinking_start") < first("text_start"));
        assert!(first("text_start") < first("toolcall_start"));
    });
}

/// An inline extension at `path` registering `shared-cmd` described as `description`,
/// whose handler records `label:<arguments>` into `calls`.
fn shared_command(
    path: &'static str,
    description: &'static str,
    label: &'static str,
    calls: &Rc<RefCell<Vec<String>>>,
) -> (&'static str, ExtensionFactory<Local>) {
    let calls = Rc::clone(calls);
    let factory: ExtensionFactory<Local> = Rc::new(move |api| {
        let calls = Rc::clone(&calls);
        let registered = api.register_command(
            "shared-cmd".into(),
            RegisteredCommandOptions {
                description: Some(description.into()),
                get_argument_completions: None,
                handler: Rc::new(move |arguments, _| {
                    calls.borrow_mut().push(format!("{label}:{arguments}"));
                    Box::pin(std::future::ready(Ok(())))
                }),
            },
        );
        Box::pin(std::future::ready(registered))
    });
    (path, factory)
}

#[test]
fn disambiguates_duplicate_extension_commands() {
    let expected = expected("inline_extension_factories");
    on_task_set(|_| async {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let h = harness(HarnessOptions {
            labelled_factories: vec![
                shared_command("<alpha>", "Alpha command", "alpha", &calls),
                shared_command("<beta>", "Beta command", "beta", &calls),
            ],
            ..HarnessOptions::default()
        })
        .await;
        let runner = h.session.extension_runner();
        let commands: Vec<Value> = runner
            .get_registered_commands()
            .into_iter()
            .map(|command| {
                json!({
                    "name": command.command.name,
                    "invocation_name": command.invocation_name,
                    "description": command.command.description,
                    "path": command.command.source_info.read().unwrap().path,
                })
            })
            .collect();
        assert_eq!(json!(commands), expected["commands"]);
        for (name, arguments) in [("shared-cmd:1", "first"), ("shared-cmd:2", "second")] {
            let command = runner.get_command(name).unwrap();
            let context = runner.create_command_context();
            (command.command.handler)(arguments.into(), context)
                .await
                .unwrap();
        }
        assert_eq!(json!(*calls.borrow()), expected["calls"]);
        assert_eq!(
            json!(h.session.has_extension_handlers("agent_start")),
            expected["has_extension_handlers_agent_start"]
        );
        assert_eq!(
            json!(h.session.has_extension_handlers("unregistered")),
            expected["has_extension_handlers_unregistered"]
        );
    });
}

#[test]
fn persists_message_entries() {
    let expected = expected("session_persistence");
    on_task_set(|_| async {
        let h = scripted(vec![Scripted::text("persisted")]).await;
        prompt(&h, "hi").await;
        let manager = h.session.session_manager().borrow();
        let types: Vec<Value> = manager
            .get_entries()
            .into_iter()
            .map(|entry| entry["type"].clone())
            .filter(|kind| kind == "message")
            .collect();
        assert_eq!(json!(types.len()), expected["message_entry_count"]);
        assert_eq!(json!(types), expected["entry_types"]);
    });
}
