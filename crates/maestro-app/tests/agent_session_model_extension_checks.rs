//! Model selection, extension hooks and lifecycle on an application session, compared with
//! the recorded cases.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    Auth, Harness, HarnessOptions, answer, echo_tools, harness, message_text, on_task_set,
    prompt_to_end, recorded,
};
use maestro_app::ModelCycleResult;
use maestro_app::Presence;
use maestro_app::agent_session::{ExtensionBindings, PromptOptions};
use maestro_extensions::{
    BeforeAgentStartEvent, BeforeAgentStartEventResult, ContextEvent, ContextEventResult,
    ExtensionFactory, InputEvent, InputEventResult, InputTransform, ModelSelectEvent,
    SessionShutdownEvent, SessionStartEvent, ToolCallEvent, ToolCallEventResult, ToolResultEvent,
    ToolResultEventResult,
};
use maestro_models::{
    AssistantMessage, DiagnosticErrorInfo, FauxAssistantMessageOptions, FauxResponseInput,
    FauxResponseStep, FauxToolCallOptions, Local, ModelThinkingLevel, StopReason, TextContent,
    Threaded, faux_assistant_message, faux_tool_call,
};
use maestro_session::session_manager::ReadonlySessionManager;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex, RwLock},
};

/// Extension observations in the order their handlers ran.
type Log = Rc<RefCell<Vec<Value>>>;

/// The two reasoning models of the selection cases.
fn reasoning_models() -> Vec<(&'static str, bool)> {
    vec![("faux-1", true), ("faux-2", true)]
}

/// The models of the scoped cycle case: the second cannot reason.
fn scoped_models() -> Vec<(&'static str, bool)> {
    vec![("faux-1", true), ("faux-2", false)]
}

/// The recorded form of a model identity.
fn model_id(model: &Arc<RwLock<maestro_models::Model>>) -> Value {
    json!(model.read().unwrap().id)
}

/// The recorded form of a thinking level.
fn level_view(level: maestro_models::ModelThinkingLevel) -> Value {
    serde_json::to_value(level).unwrap()
}

/// The recorded form of a session start reason.
fn start_label(reason: maestro_models::extension_events::events::SessionStartReason) -> Value {
    let reason = serde_json::to_value(reason).unwrap();
    json!(format!("start:{}", reason.as_str().unwrap()))
}

/// The recorded form of a session shutdown reason.
fn shutdown_label(
    reason: maestro_models::extension_events::events::SessionShutdownReason,
) -> Value {
    let reason = serde_json::to_value(reason).unwrap();
    json!(format!("shutdown:{}", reason.as_str().unwrap()))
}

/// An extension recording model selection events.
fn model_recorder(log: &Log) -> ExtensionFactory<Local> {
    let log = Rc::clone(log);
    Rc::new(move |api| {
        let log = Rc::clone(&log);
        let registered = api.on::<ModelSelectEvent, ()>(
            "model_select".into(),
            Rc::new(move |event, _| {
                let previous = event.previous_model.present().map_or_else(
                    || "none".to_owned(),
                    |model| model.read().unwrap().id.clone(),
                );
                let id = event.model.read().unwrap().id.clone();
                let source = serde_json::to_value(event.source).unwrap();
                log.borrow_mut().push(json!(format!(
                    "{previous}->{id}:{}",
                    source.as_str().unwrap()
                )));
                Box::pin(std::future::ready(Ok(None)))
            }),
        );
        Box::pin(std::future::ready(registered.map(drop)))
    })
}

/// The stored model-change entries as provider match and model identity.
fn model_changes(h: &Harness) -> Vec<Value> {
    let manager = h.session.session_manager().borrow();
    manager
        .get_entries()
        .iter()
        .filter(|entry| entry["type"] == "model_change")
        .map(|entry| {
            json!({
                "provider_matches": json!(entry["provider"] == json!("faux")),
                "model": entry["modelId"],
            })
        })
        .collect()
}

#[test]
fn model_selection_records_identity_and_event() {
    let expected = recorded(
        "model_navigation",
        "model_selection_records_identity_and_event",
    );
    on_task_set(|_| async {
        let log = Log::default();
        let h = harness(HarnessOptions {
            models: reasoning_models(),
            factories: vec![model_recorder(&log)],
            ..HarnessOptions::default()
        })
        .await;
        let next = h.model("faux-2");
        h.session.set_model(next).await.unwrap();
        let observed = json!({
            "model": model_id(&h.session.model()),
            "events": *log.borrow(),
            "changes": model_changes(&h),
        });
        assert_eq!(observed, expected);
    });
}

#[test]
fn scoped_cycle_restores_each_model_thinking_preference() {
    let expected = recorded(
        "model_navigation",
        "scoped_cycle_restores_each_model_thinking_preference",
    );
    on_task_set(|_| async {
        let h = harness(HarnessOptions {
            models: scoped_models(),
            ..HarnessOptions::default()
        })
        .await;
        h.session.set_scoped_models(vec![
            Harness::scoped(h.model("faux-1"), Some("high")),
            Harness::scoped(h.model("faux-2"), None),
        ]);
        h.session
            .set_thinking_level(ModelThinkingLevel::High)
            .unwrap();
        let mut steps = Vec::new();
        for _ in 0..2 {
            let cycled: Option<ModelCycleResult> = h
                .session
                .cycle_model(maestro_app::agent_session::CycleDirection::Forward)
                .await
                .unwrap();
            assert!(cycled.is_some());
            steps.push(json!({
                "model": model_id(&h.session.model()),
                "thinking": level_view(h.session.thinking_level()),
            }));
        }
        assert_eq!(json!(steps), expected);
    });
}

#[test]
fn non_reasoning_models_clamp_and_do_not_cycle_thinking() {
    let expected = recorded(
        "model_navigation",
        "non_reasoning_models_clamp_and_do_not_cycle_thinking",
    );
    on_task_set(|_| async {
        let h = harness(HarnessOptions {
            models: vec![("faux-1", false)],
            ..HarnessOptions::default()
        })
        .await;
        h.session
            .set_thinking_level(ModelThinkingLevel::High)
            .unwrap();
        let cycled = h.session.cycle_thinking_level().unwrap();
        let observed = json!({
            "thinking": level_view(h.session.thinking_level()),
            "cycle_absent": cycled.is_none(),
        });
        assert_eq!(observed, expected);
    });
}

#[test]
fn unauthenticated_model_change_is_rejected() {
    let expected = recorded(
        "model_navigation",
        "unauthenticated_model_change_is_rejected",
    );
    on_task_set(|_| async {
        let h = harness(HarnessOptions {
            models: reasoning_models(),
            auth: Auth::None,
            ..HarnessOptions::default()
        })
        .await;
        let error = h.session.set_model(h.model("faux-2")).await.unwrap_err();
        let observed = json!({
            "error": error.message,
            "model": model_id(&h.session.model()),
        });
        assert_eq!(observed, expected);
    });
}

/// The recorded details of a tool result: kept when it holds anything, since an empty or
/// absent record carries no information.
fn details_view(record: &Value) -> Option<Value> {
    match record.get("details") {
        None | Some(Value::Null) => None,
        Some(Value::Object(fields)) if fields.is_empty() => None,
        Some(details) => Some(details.clone()),
    }
}

/// A tool-result record as text, error flag and details.
fn result_fields(record: &Value) -> Value {
    let mut view = json!({
        "text": message_text(record),
        "is_error": record["isError"],
    });
    if let Some(details) = details_view(record) {
        view["details"] = details;
    }
    view
}

/// The tool result a request carries, as text, error flag and details.
fn tool_result_view(input: &FauxResponseInput<Local>) -> Value {
    let record = input
        .context
        .messages
        .iter()
        .map(|message| serde_json::to_value(message).unwrap())
        .find(|record| record["role"] == json!("toolResult"))
        .expect("a tool result");
    result_fields(&record)
}

/// An answer reporting the request's tool result and answering with its text.
struct AnswerToolResult(Mutex<Option<Value>>);

impl AnswerToolResult {
    /// Records the request's tool result, then answers with its text.
    fn respond(&self, input: &FauxResponseInput<Local>) -> AssistantMessage {
        let view = tool_result_view(input);
        let text = view["text"].as_str().unwrap_or_default().to_owned();
        *self.0.lock().unwrap() = Some(view);
        answer(&text, None, None)
    }
}

impl maestro_models::Invoke<Local, FauxResponseInput<Local>> for AnswerToolResult {
    type Return = <Local as maestro_models::Execution>::Future<
        'static,
        Result<AssistantMessage, DiagnosticErrorInfo>,
    >;
    fn invoke(&self, input: FauxResponseInput<Local>) -> Self::Return {
        let answer = self.respond(&input);
        Box::pin(std::future::ready(Ok(answer)))
    }
}

impl maestro_models::Invoke<Threaded, FauxResponseInput<Threaded>> for AnswerToolResult {
    type Return = <Threaded as maestro_models::Execution>::Future<
        'static,
        Result<AssistantMessage, DiagnosticErrorInfo>,
    >;
    fn invoke(&self, _: FauxResponseInput<Threaded>) -> Self::Return {
        Box::pin(std::future::ready(Ok(answer("", None, None))))
    }
}

/// The tool-call result blocking every call with its reason.
fn blocking_reason() -> ToolCallEventResult {
    serde_json::from_value(json!({"block": true, "reason": "Blocked by test"})).unwrap()
}

/// The answer calling `echo` once.
fn echo_call() -> AssistantMessage {
    let arguments = json!({"text": "hello"});
    let options = FauxToolCallOptions {
        id: Some("tool-1".into()),
    };
    faux_assistant_message(
        faux_tool_call("echo", arguments, options),
        FauxAssistantMessageOptions {
            stop_reason: Some(StopReason::ToolUse),
            ..FauxAssistantMessageOptions::default()
        },
    )
}

/// The stored tool result as text, error flag and details.
fn stored_tool_result(h: &Harness) -> Value {
    let manager = h.session.session_manager().borrow();
    let entry = manager
        .get_entries()
        .into_iter()
        .find(|entry| entry["message"]["role"] == "toolResult")
        .expect("a stored tool result");
    result_fields(&entry["message"])
}

/// The last answer's text.
fn last_answer(h: &Harness) -> Value {
    json!(h.session.get_last_assistant_text())
}

#[test]
fn tool_call_veto_reaches_the_provider_as_an_error() {
    let expected = recorded(
        "model_navigation",
        "tool_call_veto_reaches_the_provider_as_an_error",
    );
    on_task_set(|_| async {
        let executed = Rc::new(RefCell::new(false));
        let veto: ExtensionFactory<Local> = {
            Rc::new(move |api| {
                let registered = api.on::<ToolCallEvent, ToolCallEventResult>(
                    "tool_call".into(),
                    Rc::new(|_, _| Box::pin(std::future::ready(Ok(Some(blocking_reason()))))),
                );
                Box::pin(std::future::ready(registered.map(drop)))
            })
        };
        let h = harness(HarnessOptions {
            factories: vec![veto],
            base_tools_override: Some(echo_tools(Rc::clone(&executed))),
            ..HarnessOptions::default()
        })
        .await;
        let reply = Arc::new(AnswerToolResult(Mutex::new(None)));
        h.faux.set_responses(vec![
            FauxResponseStep::Message(Box::new(echo_call())),
            FauxResponseStep::Factory(reply.clone()),
        ]);
        prompt_to_end(&h, "hi", PromptOptions::default()).await;
        let ran = usize::from(*executed.borrow());
        let observed = json!({
            "executions": ran,
            "provider": reply.0.lock().unwrap().clone().expect("a request"),
            "stored": stored_tool_result(&h),
            "answer": last_answer(&h),
        });
        assert_eq!(observed, expected);
        assert_eq!(ran, 0, "the vetoed tool never ran");
    });
}

#[test]
fn tool_result_patch_reaches_history_and_next_request() {
    let expected = recorded(
        "model_navigation",
        "tool_result_patch_reaches_history_and_next_request",
    );
    on_task_set(|_| async {
        let executed = Rc::new(RefCell::new(false));
        let patch: ExtensionFactory<Local> = Rc::new(|api| {
            let registered = api.on::<ToolResultEvent, ToolResultEventResult>(
                "tool_result".into(),
                Rc::new(|_, _| {
                    let result: ToolResultEventResult = serde_json::from_value(json!({
                        "content": [{"type": "text", "text": "patched result"}],
                        "details": {"patched": true},
                    }))
                    .unwrap();
                    Box::pin(std::future::ready(Ok(Some(result))))
                }),
            );
            Box::pin(std::future::ready(registered.map(drop)))
        });
        let h = harness(HarnessOptions {
            factories: vec![patch],
            base_tools_override: Some(echo_tools(Rc::clone(&executed))),
            ..HarnessOptions::default()
        })
        .await;
        let reply = Arc::new(AnswerToolResult(Mutex::new(None)));
        h.faux.set_responses(vec![
            FauxResponseStep::Message(Box::new(echo_call())),
            FauxResponseStep::Factory(reply.clone()),
        ]);
        prompt_to_end(&h, "hi", PromptOptions::default()).await;
        let observed = json!({
            "executions": usize::from(*executed.borrow()),
            "provider": reply.0.lock().unwrap().clone().expect("a request"),
            "stored": stored_tool_result(&h),
            "answer": last_answer(&h),
        });
        assert_eq!(observed, expected);
    });
}

/// An answer reporting the request's first user text.
struct AnswerUserText(Mutex<Option<String>>);

impl AnswerUserText {
    /// Records the request's first user text and answers `done`.
    fn respond(&self, input: &FauxResponseInput<Local>) -> AssistantMessage {
        let text = input
            .context
            .messages
            .iter()
            .map(|message| serde_json::to_value(message).unwrap())
            .find(|record| record["role"] == json!("user"))
            .map(|record| {
                message_text(&record)
                    .as_str()
                    .unwrap_or_default()
                    .to_owned()
            })
            .unwrap_or_default();
        *self.0.lock().unwrap() = Some(text);
        answer("done", None, None)
    }
}

impl maestro_models::Invoke<Local, FauxResponseInput<Local>> for AnswerUserText {
    type Return = <Local as maestro_models::Execution>::Future<
        'static,
        Result<AssistantMessage, DiagnosticErrorInfo>,
    >;
    fn invoke(&self, input: FauxResponseInput<Local>) -> Self::Return {
        let answer = self.respond(&input);
        Box::pin(std::future::ready(Ok(answer)))
    }
}

impl maestro_models::Invoke<Threaded, FauxResponseInput<Threaded>> for AnswerUserText {
    type Return = <Threaded as maestro_models::Execution>::Future<
        'static,
        Result<AssistantMessage, DiagnosticErrorInfo>,
    >;
    fn invoke(&self, _: FauxResponseInput<Threaded>) -> Self::Return {
        Box::pin(std::future::ready(Ok(answer("", None, None))))
    }
}

/// `message` as the recorded form its rewrite replaces the user content of.
fn rewritten(message: &maestro_extensions::NativeAgentMessage) -> Value {
    let mut record = serde_json::to_value(message).unwrap();
    if record["role"] == json!("user") {
        record["content"] = json!([{"type": "text", "text": "rewritten"}]);
    }
    record
}

/// What the input handler decides for `text`: a handled ping, every other input transformed.
fn input_decision(text: &str) -> InputEventResult {
    if text == "ping" {
        return InputEventResult::Handled;
    }
    InputEventResult::Transform(InputTransform {
        text: format!("transformed:{text}"),
        images: Presence::Missing,
    })
}

/// The stored user messages as text.
fn stored_users(h: &Harness) -> Vec<Value> {
    h.session
        .messages()
        .read()
        .unwrap()
        .iter()
        .map(|message| serde_json::to_value(message).unwrap())
        .filter(|record| record["role"] == json!("user"))
        .map(|record| message_text(&record))
        .collect()
}

#[test]
fn context_rewrite_does_not_overwrite_saved_user_input() {
    let expected = recorded(
        "model_navigation",
        "context_rewrite_does_not_overwrite_saved_user_input",
    );
    on_task_set(|_| async {
        let rewrite: ExtensionFactory<Local> = Rc::new(|api| {
            let registered = api.on::<ContextEvent, ContextEventResult>(
                "context".into(),
                Rc::new(|event, _| {
                    let messages: Vec<Value> = event.messages.iter().map(rewritten).collect();
                    let result: ContextEventResult =
                        serde_json::from_value(json!({"messages": messages})).unwrap();
                    Box::pin(std::future::ready(Ok(Some(result))))
                }),
            );
            Box::pin(std::future::ready(registered.map(drop)))
        });
        let h = harness(HarnessOptions {
            factories: vec![rewrite],
            ..HarnessOptions::default()
        })
        .await;
        let answer = Arc::new(AnswerUserText(Mutex::new(None)));
        h.faux.set_responses(vec![
            FauxResponseStep::Factory(answer.clone()),
            FauxResponseStep::Message(Box::new(maestro_models::faux_assistant_message(
                TextContent::new("done".into()),
                FauxAssistantMessageOptions::default(),
            ))),
        ]);
        prompt_to_end(&h, "original", PromptOptions::default()).await;
        let observed = json!({
            "provider": answer.0.lock().unwrap().clone(),
            "stored": stored_users(&h).first().cloned(),
        });
        assert_eq!(observed, expected);
    });
}

#[test]
fn input_transform_and_handled_paths_are_distinct() {
    let expected = recorded(
        "model_navigation",
        "input_transform_and_handled_paths_are_distinct",
    );
    on_task_set(|_| async {
        let transforming: ExtensionFactory<Local> = Rc::new(|api| {
            let registered = api.on::<InputEvent, InputEventResult>(
                "input".into(),
                Rc::new(|event, _| {
                    Box::pin(std::future::ready(Ok(Some(input_decision(&event.text)))))
                }),
            );
            Box::pin(std::future::ready(registered.map(drop)))
        });
        let h = harness(HarnessOptions {
            factories: vec![transforming],
            ..HarnessOptions::default()
        })
        .await;
        let answer = Arc::new(AnswerUserText(Mutex::new(None)));
        h.faux.set_responses(vec![
            FauxResponseStep::Factory(answer.clone()),
            FauxResponseStep::Message(Box::new(maestro_models::faux_assistant_message(
                TextContent::new("done".into()),
                FauxAssistantMessageOptions::default(),
            ))),
        ]);
        prompt_to_end(&h, "hello", PromptOptions::default()).await;
        // A handled input starts no run, so the prompt returns once it is handled.
        h.session
            .prompt("ping", PromptOptions::default())
            .await
            .unwrap();
        let observed = json!({
            "provider": answer.0.lock().unwrap().clone(),
            "user_count": stored_users(&h).len(),
            "calls": h.faux.state.call_count(),
        });
        assert_eq!(observed, expected);
    });
}

#[test]
fn before_start_injection_reaches_prompt_and_history() {
    let expected = recorded(
        "model_navigation",
        "before_start_injection_reaches_prompt_and_history",
    );
    on_task_set(|_| async {
        let injecting: ExtensionFactory<Local> = Rc::new(|api| {
            let registered = api.on::<BeforeAgentStartEvent, BeforeAgentStartEventResult>(
                "before_agent_start".into(),
                Rc::new(|event, _| {
                    let message = json!({
                        "customType": "before-start",
                        "content": "injected",
                        "display": true,
                        "details": {"injected": true},
                    });
                    let result: BeforeAgentStartEventResult = serde_json::from_value(json!({
                        "message": message,
                        "systemPrompt": format!("{}\n\nextra instructions", event.system_prompt),
                    }))
                    .unwrap();
                    Box::pin(std::future::ready(Ok(Some(result))))
                }),
            );
            Box::pin(std::future::ready(registered.map(drop)))
        });
        let h = harness(HarnessOptions {
            factories: vec![injecting],
            ..HarnessOptions::default()
        })
        .await;
        let seen = Arc::new(AnswerSystemPrompt(Mutex::new(None)));
        h.faux.set_responses(vec![
            FauxResponseStep::Factory(seen.clone()),
            FauxResponseStep::Message(Box::new(maestro_models::faux_assistant_message(
                TextContent::new("done".into()),
                FauxAssistantMessageOptions::default(),
            ))),
        ]);
        prompt_to_end(&h, "hello", PromptOptions::default()).await;
        let manager = h.session.session_manager().borrow();
        let custom = manager
            .get_entries()
            .into_iter()
            .find(|entry| entry["type"] == json!("custom_message"))
            .expect("a stored custom message");
        let observed = json!({
            "provider": seen.0.lock().unwrap().clone().expect("a request"),
            "stored": {
                "customType": custom["customType"],
                "content": custom["content"],
                "display": custom["display"],
                "details": custom["details"],
            },
        });
        assert_eq!(observed, expected);
    });
}

/// An answer reporting whether the request carries the extra instructions and the injected
/// message, then answering `done`.
struct AnswerSystemPrompt(Mutex<Option<Value>>);

impl AnswerSystemPrompt {
    /// Records what the request carries and answers `done`.
    fn respond(&self, input: &FauxResponseInput<Local>) -> AssistantMessage {
        let prompt = input.context.system_prompt.clone().unwrap_or_default();
        let injected = input
            .context
            .messages
            .iter()
            .map(|message| serde_json::to_value(message).unwrap())
            .any(|record| {
                record["role"] == json!("user") && message_text(&record) == json!("injected")
            });
        *self.0.lock().unwrap() = Some(json!({
            "extra_instructions": prompt.contains("extra instructions"),
            "injected": injected,
        }));
        answer("done", None, None)
    }
}

impl maestro_models::Invoke<Local, FauxResponseInput<Local>> for AnswerSystemPrompt {
    type Return = <Local as maestro_models::Execution>::Future<
        'static,
        Result<AssistantMessage, DiagnosticErrorInfo>,
    >;
    fn invoke(&self, input: FauxResponseInput<Local>) -> Self::Return {
        let answer = self.respond(&input);
        Box::pin(std::future::ready(Ok(answer)))
    }
}

impl maestro_models::Invoke<Threaded, FauxResponseInput<Threaded>> for AnswerSystemPrompt {
    type Return = <Threaded as maestro_models::Execution>::Future<
        'static,
        Result<AssistantMessage, DiagnosticErrorInfo>,
    >;
    fn invoke(&self, _: FauxResponseInput<Threaded>) -> Self::Return {
        Box::pin(std::future::ready(Ok(answer("done", None, None))))
    }
}

/// An extension recording startup and shutdown reasons in delivery order.
fn lifecycle_recorder(seen: &Log) -> ExtensionFactory<Local> {
    let seen = Rc::clone(seen);
    Rc::new(move |api| {
        let start_seen = Rc::clone(&seen);
        let started = api.on::<SessionStartEvent, ()>(
            "session_start".into(),
            Rc::new(move |event, _| {
                start_seen.borrow_mut().push(start_label(event.reason));
                Box::pin(std::future::ready(Ok(None)))
            }),
        );
        let shutdown_seen = Rc::clone(&seen);
        let stopped = api.on::<SessionShutdownEvent, ()>(
            "session_shutdown".into(),
            Rc::new(move |event, _| {
                shutdown_seen
                    .borrow_mut()
                    .push(shutdown_label(event.reason));
                Box::pin(std::future::ready(Ok(None)))
            }),
        );
        Box::pin(std::future::ready(started.and(stopped).map(drop)))
    })
}

#[test]
fn bound_reload_orders_shutdown_before_restart() {
    let expected = recorded(
        "model_navigation",
        "bound_reload_orders_shutdown_before_restart",
    );
    on_task_set(|_| async {
        let seen = Log::default();
        let h = harness(HarnessOptions {
            factories: vec![lifecycle_recorder(&seen)],
            ..HarnessOptions::default()
        })
        .await;
        h.session
            .bind_extensions(ExtensionBindings {
                shutdown_handler: Some(Rc::new(|| Ok(()))),
                ..ExtensionBindings::default()
            })
            .await
            .unwrap();
        h.session.reload().await.unwrap();
        assert_eq!(json!(*seen.borrow()), expected);
    });
}
