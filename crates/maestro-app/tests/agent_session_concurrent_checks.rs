//! Prompts while a response streams, both message queues, extension-origin steering, tool
//! hooks and slow message handlers against the stored history, a complete tool exchange with
//! its cleanup, and a session over empty resource roots, compared with the recorded
//! concurrency cases.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    Harness, HarnessOptions, answer, corpus, get_assistant_texts, get_message_text, get_user_texts,
    harness, on_task_set, prompt_to_end, session_run_ended, start_prompt,
};
use indexmap::IndexMap;
use maestro_agent::{AgentEvent, AgentMessage, AgentToolRecord, AgentToolResult, SharedAgentTool};
use maestro_app::{AgentSessionEvent, PromptOptions};
use maestro_extensions::{
    ExtensionAPI, ExtensionFactory, InputEvent, InputEventResult, MessageEndEvent,
    MessageEndEventResult, NativeAgentMessage, ToolCallEvent, ToolCallEventResult,
};
use maestro_models::{
    AssistantContent, AssistantMessage, Cancellation, DiagnosticErrorInfo,
    FauxAssistantMessageOptions, FauxResponseInput, FauxResponseStep, FauxToolCallOptions,
    ImageContent, Local, ProviderExecution, StopReason, StreamOptionsInput, TextContent, Threaded,
    Tool, UserBlock, faux_assistant_message, faux_tool_call,
};
use maestro_session::session_manager::ReadonlySessionManager;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex, RwLock},
};
use tokio::sync::Notify;

/// The recorded concurrency case `name`.
fn expected(name: &str) -> Value {
    corpus("concurrent")[name].clone()
}

/// The roles of the stored message entries, in order.
fn stored_roles(manager: &dyn ReadonlySessionManager) -> Vec<String> {
    manager
        .get_entries()
        .iter()
        .filter(|entry| entry["type"] == "message")
        .map(|entry| entry["message"]["role"].as_str().unwrap().to_owned())
        .collect()
}

/// The roles of the session's stored message entries.
fn roles(h: &Harness) -> Vec<String> {
    stored_roles(&*h.session.session_manager().borrow())
}

/// The cancellation a provider request carries.
fn request_signal<E: ProviderExecution>(input: &FauxResponseInput<E>) -> Cancellation {
    let common = match input.options.as_ref().unwrap() {
        StreamOptionsInput::Raw(options) => &options.common,
        StreamOptionsInput::Simple(options) => &options.common,
    };
    common.signal.clone().unwrap()
}

/// What a held answer reports: that its request arrived, and the request's cancellation.
#[derive(Clone, Default)]
struct Held {
    /// Notified when the provider is asked.
    requested: Arc<Notify>,
    /// The request's cancellation, once asked.
    signal: Arc<Mutex<Option<Cancellation>>>,
}

/// A scripted answer recording the roles of each request's conversation; a held one keeps
/// the request's cancellation, reports the request and answers only once it is cancelled.
struct ObservedAnswer {
    /// The roles of each request's conversation, in request order.
    contexts: Arc<Mutex<Vec<Vec<String>>>>,
    /// Present for an answer that waits for its request's cancellation.
    held: Option<Held>,
    /// The answer.
    response: AssistantMessage,
}

impl ObservedAnswer {
    /// Records the request, then answers, after its cancellation when held.
    fn respond<E: ProviderExecution>(
        &self,
        input: &FauxResponseInput<E>,
    ) -> impl Future<Output = Result<AssistantMessage, DiagnosticErrorInfo>> + Send + 'static {
        let roles = input.context.messages.iter().map(|message| {
            let record = serde_json::to_value(message).unwrap();
            record["role"].as_str().unwrap().to_owned()
        });
        self.contexts.lock().unwrap().push(roles.collect());
        let signal = self.held.as_ref().map(|held| {
            let signal = request_signal(input);
            *held.signal.lock().unwrap() = Some(signal.clone());
            held.requested.notify_one();
            signal
        });
        let response = self.response.clone();
        async move {
            if let Some(signal) = signal {
                signal.cancelled().await;
            }
            Ok(response)
        }
    }
}

impl maestro_models::Invoke<Local, FauxResponseInput<Local>> for ObservedAnswer {
    type Return = <Local as maestro_models::Execution>::Future<
        'static,
        Result<AssistantMessage, DiagnosticErrorInfo>,
    >;
    fn invoke(&self, input: FauxResponseInput<Local>) -> Self::Return {
        Box::pin(self.respond(&input))
    }
}

impl maestro_models::Invoke<Threaded, FauxResponseInput<Threaded>> for ObservedAnswer {
    type Return = <Threaded as maestro_models::Execution>::Future<
        'static,
        Result<AssistantMessage, DiagnosticErrorInfo>,
    >;
    fn invoke(&self, input: FauxResponseInput<Threaded>) -> Self::Return {
        Box::pin(self.respond(&input))
    }
}

/// The scripted step answering `response` and recording its request in `contexts`.
fn observed(
    contexts: &Arc<Mutex<Vec<Vec<String>>>>,
    response: AssistantMessage,
) -> FauxResponseStep {
    FauxResponseStep::Factory(Arc::new(ObservedAnswer {
        contexts: Arc::clone(contexts),
        held: None,
        response,
    }))
}

/// A text result block.
fn text_block(text: String) -> UserBlock {
    UserBlock::Text(TextContent::new(text))
}

/// The `dummy` tool: it reports `working:<q>` progress, then answers `result:<q>`, or fails
/// with `tool failed` when `q` is `failing`.
fn dummy_tools(failing: Option<&'static str>) -> IndexMap<String, SharedAgentTool<Local>> {
    let tool = AgentToolRecord::<Local> {
        definition: Tool {
            name: "dummy".into(),
            description: "Dummy tool".into(),
            parameters: json!({"type": "object", "properties": {"q": {"type": "string"}},
                "required": ["q"]})
            .into(),
        },
        description: None,
        label: "dummy".into(),
        prepare_arguments: None,
        execute: Rc::new(move |_, arguments, _, on_update| {
            let q = arguments["q"].as_str().unwrap().to_owned();
            if let Some(update) = on_update {
                update(AgentToolResult {
                    content: vec![text_block(format!("working:{q}"))],
                    details: Some(json!({"phase": "working"})),
                    terminate: None,
                    usage: None,
                });
            }
            let result = if failing == Some(q.as_str()) {
                Err(DiagnosticErrorInfo::new(None, "tool failed"))
            } else {
                Ok(AgentToolResult {
                    content: vec![text_block(format!("result:{q}"))],
                    details: Some(json!({})),
                    terminate: None,
                    usage: None,
                })
            };
            Box::pin(std::future::ready(result))
        }),
        execution_mode: None,
    };
    IndexMap::from([("dummy".to_owned(), Rc::new(RwLock::new(tool)))])
}

/// An answer calling `dummy` once per `(q, id)` pair, after an optional text.
fn dummy_calls(text: Option<&str>, calls: &[(&str, &str)]) -> AssistantMessage {
    let mut content: Vec<AssistantContent> = text
        .map(|text| AssistantContent::Text(TextContent::new(text.into())))
        .into_iter()
        .collect();
    content.extend(calls.iter().map(|(q, id)| {
        let options = FauxToolCallOptions {
            id: Some((*id).into()),
        };
        AssistantContent::ToolCall(faux_tool_call("dummy", json!({"q": q}), options))
    }));
    let options = FauxAssistantMessageOptions {
        stop_reason: Some(StopReason::ToolUse),
        ..FauxAssistantMessageOptions::default()
    };
    faux_assistant_message(content, options)
}

/// The supplied thinking and code answer, as its fixture records it.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordedAnswer {
    /// The answer's content blocks.
    content: Value,
}

/// The content blocks of the supplied thinking and code answer.
fn thinking_code_content() -> Value {
    let path = format!(
        "{}/tests/fixtures/assistant-message-with-thinking-code.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(path).unwrap();
    serde_json::from_str::<RecordedAnswer>(&text)
        .unwrap()
        .content
}

/// Each tool's progress reports as the session delivers them: call id, content and details.
fn tool_progress(h: &Harness) -> Rc<RefCell<Vec<Value>>> {
    let progress = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&progress);
    drop(h.session.subscribe(Rc::new(move |event| {
        if let AgentSessionEvent::Agent(event) = event
            && let AgentEvent::ToolExecutionUpdate {
                tool_call,
                partial_result,
            } = &*event.read().unwrap()
        {
            recorded.borrow_mut().push(json!({
                "id": tool_call.id,
                "content": partial_result.content,
                "details": partial_result.details,
            }));
        }
    })));
    progress
}

/// The tool results in the session history: call id, content and whether it failed.
fn tool_results(h: &Harness) -> Vec<Value> {
    let messages = h.session.messages();
    let messages = messages.read().unwrap();
    messages
        .iter()
        .map(|message| serde_json::to_value(message).unwrap())
        .filter(|record| record["role"] == "toolResult")
        .map(|record| {
            json!({"id": record["toolCallId"], "content": record["content"],
                "is_error": record["isError"]})
        })
        .collect()
}

/// The content of the newest history message and of the newest stored message.
fn final_contents(h: &Harness) -> (Value, Value) {
    let messages = h.session.messages();
    let last = serde_json::to_value(messages.read().unwrap().last().unwrap()).unwrap();
    let manager = h.session.session_manager().borrow();
    let entries = manager.get_entries();
    let stored = entries
        .iter()
        .rev()
        .find(|entry| entry["type"] == "message");
    (
        last["content"].clone(),
        stored.unwrap()["message"]["content"].clone(),
    )
}

/// The block types of the first stored message.
fn first_stored_block_types(h: &Harness) -> Vec<Value> {
    let manager = h.session.session_manager().borrow();
    let entries = manager.get_entries();
    let first = entries.iter().find(|entry| entry["type"] == "message");
    let blocks = first.unwrap()["message"]["content"]
        .as_array()
        .unwrap()
        .clone();
    blocks
        .into_iter()
        .map(|block| block["type"].clone())
        .collect()
}

/// Disposes the session and drops the harness, checking that a context kept from the session
/// goes stale on disposal, and that the scripted provider and the disposable root remain until
/// the drop and are gone after it.
fn assert_released(h: Harness) {
    let context = h.runner().create_context();
    let api = h.faux.api.clone();
    let root = h.scratch.path().to_owned();
    assert!(context.cwd().is_ok());
    h.session.dispose();
    assert!(context.cwd().is_err());
    assert!(maestro_models::get_api_provider(&api).is_some());
    assert!(root.exists());
    drop(h);
    assert!(maestro_models::get_api_provider(&api).is_none());
    assert!(!root.exists());
}

#[test]
fn dropping_the_harness_disposes_the_session() {
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let context = h.runner().create_context();
        assert!(context.cwd().is_ok());
        drop(h);
        assert!(
            context.cwd().is_err(),
            "the session disposed with its harness"
        );
    });
}

#[test]
fn controlled_exchange_retains_content_results_and_progress() {
    let expected = expected("exchange");
    let content = thinking_code_content();
    on_task_set(|_| async {
        let h = harness(HarnessOptions {
            base_tools_override: Some(dummy_tools(Some("y"))),
            ..HarnessOptions::default()
        })
        .await;
        let blocks: Vec<AssistantContent> = serde_json::from_value(content.clone()).unwrap();
        let contexts = Arc::default();
        let calls = dummy_calls(None, &[("x", "toolu_1"), ("y", "toolu_2")]);
        h.faux.set_responses(vec![observed(&contexts, calls)]);
        let fixture = faux_assistant_message(blocks, FauxAssistantMessageOptions::default());
        h.faux.append_responses(vec![observed(&contexts, fixture)]);
        let queued = h.faux.get_pending_response_count();
        let progress = tool_progress(&h);
        let image = ImageContent {
            data: "AA==".into(),
            mime_type: "image/png".into(),
        };
        let options = PromptOptions {
            images: Some(vec![image]),
            ..PromptOptions::default()
        };
        prompt_to_end(&h, "hi", options).await;
        let observed = json!({
            "queued": queued, "pending": h.faux.get_pending_response_count(),
            "contexts": *contexts.lock().unwrap(), "roles": roles(&h), "results": tool_results(&h),
            "progress": *progress.borrow(), "users": get_user_texts(&h),
        });
        assert_eq!(observed, expected);
        assert_eq!(
            first_stored_block_types(&h),
            [json!("text"), json!("image")]
        );
        assert_eq!(final_contents(&h), (content.clone(), content));
        assert_released(h);
    });
}

/// What the test sends while the first response is held.
#[derive(Clone, Copy)]
enum WhileStreaming {
    /// An ordinary second prompt.
    Prompt,
    /// A steering message.
    Steer,
    /// A follow-up message.
    FollowUp,
}

/// The streaming flag, the pending count and both queue mirrors in their recorded form.
fn queue_state(h: &Harness) -> Value {
    json!({
        "streaming": h.session.is_streaming(),
        "pending": h.session.pending_message_count(),
        "steering": h.session.get_steering_messages(),
        "follow_up": h.session.get_follow_up_messages(),
    })
}

/// The held answer to `First message`, then `Steered` should the session ever ask again.
fn hold_first_request(h: &Harness) -> Held {
    let held = Held::default();
    let first = ObservedAnswer {
        contexts: Arc::default(),
        held: Some(held.clone()),
        response: answer("Aborted", None, None),
    };
    h.faux.set_responses(vec![
        FauxResponseStep::Factory(Arc::new(first)),
        FauxResponseStep::Message(Box::new(answer("Steered", None, None))),
    ]);
    held
}

/// Sends `second` while `First message` is held, then aborts; the recorded form of the state
/// after sending and after the run ended.
async fn send_while_streaming(second: WhileStreaming) -> Value {
    let h = harness(HarnessOptions::default()).await;
    let held = hold_first_request(&h);
    let ended = session_run_ended(&h);
    let running = start_prompt(&h, "First message");
    held.requested.notified().await;
    assert!(h.session.is_streaming());
    let error = match second {
        WhileStreaming::Prompt => {
            let refused = h.session.prompt("Second message", PromptOptions::default());
            Some(refused.await.unwrap_err().message)
        }
        WhileStreaming::Steer => {
            h.session.steer("Steering message", &[]).unwrap();
            None
        }
        WhileStreaming::FollowUp => {
            h.session.follow_up("Follow-up message", &[]).unwrap();
            None
        }
    };
    let mut during = queue_state(&h);
    if let Some(error) = error {
        during["error"] = json!(error);
    }
    h.session.abort().await;
    running.await.unwrap().unwrap();
    h.session.agent().wait_for_idle().await;
    ended.notified().await;
    let signal = held.signal.lock().unwrap().clone().unwrap();
    let after = json!({
        "streaming": h.session.is_streaming(), "signal_aborted": signal.is_aborted(),
        "users": get_user_texts(&h), "assistants": get_assistant_texts(&h), "roles": roles(&h),
        "calls": h.faux.state.call_count(),
    });
    json!({"during": during, "after": after})
}

#[test]
fn busy_prompt_reports_queue_guidance() {
    let expected = expected("busy");
    on_task_set(|_| async {
        assert_eq!(send_while_streaming(WhileStreaming::Prompt).await, expected);
    });
}

#[test]
fn steering_remains_available_during_streaming() {
    let expected = expected("steer");
    on_task_set(|_| async {
        assert_eq!(send_while_streaming(WhileStreaming::Steer).await, expected);
    });
}

#[test]
fn follow_up_remains_available_during_streaming() {
    let expected = expected("follow_up");
    on_task_set(|_| async {
        assert_eq!(
            send_while_streaming(WhileStreaming::FollowUp).await,
            expected
        );
    });
}

/// The extension keeping its interface in `api` and the source of each input in `sources`.
fn steering_extension(
    api: &Rc<RefCell<Option<ExtensionAPI<Local>>>>,
    sources: &Rc<RefCell<Vec<Value>>>,
) -> ExtensionFactory<Local> {
    let api = Rc::clone(api);
    let sources = Rc::clone(sources);
    Rc::new(move |registered| {
        let sources = Rc::clone(&sources);
        let handled = registered.on::<InputEvent, InputEventResult>(
            "input".into(),
            Rc::new(move |event, _| {
                sources
                    .borrow_mut()
                    .push(serde_json::to_value(event.source).unwrap());
                Box::pin(std::future::ready(Ok(None)))
            }),
        );
        *api.borrow_mut() = Some(registered);
        Box::pin(std::future::ready(handled.map(drop)))
    })
}

/// Replaces the session's stream, recording each request's user texts in `requests`. A
/// request holding `Steer from extension` answers `Steered`; any other reports itself through
/// `held` and, once cancelled, fails as aborted with an answer that stopped normally, so the
/// queued steering message still runs.
fn steering_stream(h: &Harness, held: &Held, requests: &Rc<RefCell<Vec<Vec<String>>>>) {
    use maestro_models::{AssistantMessageEvent as Event, DoneReason, ErrorReason};
    let held = held.clone();
    let requests = Rc::clone(requests);
    *h.session.agent().stream_fn.write().unwrap() = Some(Rc::new(move |_, context, options, _| {
        let users: Vec<String> = context
            .messages
            .iter()
            .map(|message| serde_json::to_value(message).unwrap())
            .filter(|record| record["role"] == "user")
            .map(|record| get_message_text(&record))
            .collect();
        let steered = users.iter().any(|text| text == "Steer from extension");
        requests.borrow_mut().push(users);
        let stream = maestro_models::AssistantMessageEventStream::new();
        let shared = |text| Arc::new(RwLock::new(answer(text, None, None)));
        stream.push(Event::Start {
            partial: shared(""),
        });
        if steered {
            let message = shared("Steered");
            stream.push(Event::Done {
                reason: DoneReason::Stop,
                message,
            });
        } else {
            let signal = options.common.signal.unwrap();
            held.signal
                .lock()
                .unwrap()
                .get_or_insert_with(|| signal.clone());
            held.requested.notify_one();
            let worker = stream.clone();
            tokio::task::spawn_local(async move {
                signal.cancelled().await;
                let error = shared("Aborted");
                worker.push(Event::Error {
                    reason: ErrorReason::Aborted,
                    error,
                });
            });
        }
        Box::pin(std::future::ready(Ok(stream)))
    }));
}

/// Each queue update the session delivers, and a notifier for the one that first shows
/// `Steer from extension` as steering.
fn queue_updates(h: &Harness) -> (Rc<RefCell<Vec<Value>>>, Rc<Notify>) {
    let updates = Rc::new(RefCell::new(Vec::new()));
    let queued = Rc::new(Notify::new());
    let (recorded, notify) = (Rc::clone(&updates), Rc::clone(&queued));
    drop(h.session.subscribe(Rc::new(move |event| {
        if let AgentSessionEvent::QueueUpdate {
            steering,
            follow_up,
        } = event
        {
            recorded
                .borrow_mut()
                .push(json!({"steering": steering, "followUp": follow_up}));
            if steering.iter().any(|text| text == "Steer from extension") {
                notify.notify_one();
            }
        }
    })));
    (updates, queued)
}

#[test]
fn extension_steering_reaches_the_next_provider_request() {
    use maestro_session::messages::{SendUserMessageOptions, UserMessageDelivery};
    let expected = expected("extension");
    on_task_set(|_| async {
        let (api, sources) = (Rc::default(), Rc::default());
        let h = harness(HarnessOptions {
            factories: vec![steering_extension(&api, &sources)],
            ..HarnessOptions::default()
        })
        .await;
        let (held, requests) = (Held::default(), Rc::default());
        steering_stream(&h, &held, &requests);
        let (updates, queued) = queue_updates(&h);
        let ended = session_run_ended(&h);
        let running = start_prompt(&h, "First message");
        held.requested.notified().await;
        assert!(h.session.is_streaming());
        let api = api.borrow_mut().take().unwrap();
        let options = SendUserMessageOptions {
            deliver_as: Some(UserMessageDelivery::Steer),
        };
        let content = maestro_models::UserContent::Text("Steer from extension".into());
        api.send_user_message(content, Some(options)).unwrap();
        queued.notified().await;
        let mut during = queue_state(&h);
        during["source"] = sources.borrow().last().cloned().unwrap();
        during["updates"] = json!(*updates.borrow());
        h.session.abort().await;
        running.await.unwrap().unwrap();
        h.session.agent().wait_for_idle().await;
        ended.notified().await;
        let signal = held.signal.lock().unwrap().clone().unwrap();
        let after = json!({
            "streaming": h.session.is_streaming(), "signal_aborted": signal.is_aborted(),
            "users": get_user_texts(&h), "assistants": get_assistant_texts(&h), "roles": roles(&h),
            "requests": *requests.borrow(),
        });
        assert_eq!(json!({"during": during, "after": after}), expected);
    });
}

#[test]
fn completed_prompts_allow_another_run() {
    let expected = expected("sequential");
    let blocks: Vec<AssistantContent> = serde_json::from_value(json!([
        {"type": "text", "text": ""}, {"type": "thinking", "thinking": "not displayed"},
        {"type": "text", "text": "\u{feff}\u{85}\u{1f600}"}, {"type": "text", "text": "e\u{301}"},
        {"type": "text", "text": ""},
    ]))
    .unwrap();
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        h.faux.set_responses(vec![
            FauxResponseStep::Message(Box::new(answer("Done", None, None))),
            FauxResponseStep::Message(Box::new(answer("Again", None, None))),
        ]);
        let ended = session_run_ended(&h);
        h.session
            .prompt("First message", PromptOptions::default())
            .await
            .unwrap();
        assert!(!h.session.is_streaming());
        ended.notified().await;
        prompt_to_end(&h, "Second message", PromptOptions::default()).await;
        let blocks = faux_assistant_message(blocks, FauxAssistantMessageOptions::default());
        h.faux
            .append_responses(vec![FauxResponseStep::Message(Box::new(blocks))]);
        prompt_to_end(&h, "Third message", PromptOptions::default()).await;
        let observed = json!({
            "streaming": h.session.is_streaming(), "users": get_user_texts(&h),
            "assistants": get_assistant_texts(&h), "roles": roles(&h),
        });
        assert_eq!(observed, expected);
    });
}

/// Each tool call's id and the roles stored when its hook ran, in call order.
type Snapshots = Rc<RefCell<Vec<(String, Vec<String>)>>>;

/// The extension recording, at each tool call, the call id and the roles stored by then.
fn tool_call_snapshots(snapshots: &Snapshots) -> ExtensionFactory<Local> {
    let snapshots = Rc::clone(snapshots);
    Rc::new(move |api| {
        let snapshots = Rc::clone(&snapshots);
        let registered = api.on::<ToolCallEvent, ToolCallEventResult>(
            "tool_call".into(),
            Rc::new(move |event, context| {
                let manager = context.session_manager().unwrap();
                let roles = stored_roles(&*manager.borrow());
                snapshots
                    .borrow_mut()
                    .push((event.tool_call_id.clone(), roles));
                Box::pin(std::future::ready(Ok(None)))
            }),
        );
        Box::pin(std::future::ready(registered.map(drop)))
    })
}

#[test]
fn tool_hooks_observe_both_persisted_messages() {
    let expected = expected("tool_queue");
    on_task_set(|_| async {
        let snapshots = Rc::default();
        let h = harness(HarnessOptions {
            factories: vec![tool_call_snapshots(&snapshots)],
            base_tools_override: Some(dummy_tools(None)),
            ..HarnessOptions::default()
        })
        .await;
        h.faux.set_responses(vec![
            FauxResponseStep::Message(Box::new(dummy_calls(
                None,
                &[("x", "toolu_1"), ("y", "toolu_2")],
            ))),
            FauxResponseStep::Message(Box::new(answer("done", None, None))),
        ]);
        prompt_to_end(&h, "hi", PromptOptions::default()).await;
        let (ids, snapshots): (Vec<String>, Vec<Vec<String>>) =
            snapshots.borrow().iter().cloned().unzip();
        assert_eq!(ids, ["toolu_1", "toolu_2"]);
        let observed = json!({"snapshots": snapshots, "roles": roles(&h)});
        assert_eq!(observed, expected);
    });
}

/// When holding, reports the hold's notifier, then waits until its release turns true.
async fn hold_until_released(hold: Option<(Rc<Notify>, tokio::sync::watch::Receiver<bool>)>) {
    if let Some((entered, mut release)) = hold {
        entered.notify_one();
        drop(release.wait_for(|released| *released).await.unwrap());
    }
}

/// The extension whose `message_end` handler, for an answer, reports `entered` and then
/// waits until `release` turns true.
fn slow_answer_handler(
    entered: &Rc<Notify>,
    release: &tokio::sync::watch::Receiver<bool>,
) -> ExtensionFactory<Local> {
    let (entered, release) = (Rc::clone(entered), release.clone());
    Rc::new(move |api| {
        let (entered, release) = (Rc::clone(&entered), release.clone());
        let registered = api.on::<MessageEndEvent, MessageEndEventResult<NativeAgentMessage>>(
            "message_end".into(),
            Rc::new(move |event, _| {
                let answer = matches!(event.message, AgentMessage::Assistant(_));
                let hold = answer.then(|| (Rc::clone(&entered), release.clone()));
                Box::pin(async move {
                    hold_until_released(hold).await;
                    Ok(None)
                })
            }),
        );
        Box::pin(std::future::ready(registered.map(drop)))
    })
}

#[test]
fn slow_message_handlers_preserve_history_order() {
    let expected = expected("slow_handler");
    on_task_set(|_| async {
        let entered = Rc::new(Notify::new());
        let (release, released) = tokio::sync::watch::channel(false);
        let h = harness(HarnessOptions {
            factories: vec![slow_answer_handler(&entered, &released)],
            base_tools_override: Some(dummy_tools(None)),
            ..HarnessOptions::default()
        })
        .await;
        h.faux.set_responses(vec![
            FauxResponseStep::Message(Box::new(dummy_calls(
                Some("calling tool"),
                &[("x", "toolu_1")],
            ))),
            FauxResponseStep::Message(Box::new(answer("done", None, None))),
        ]);
        let ended = session_run_ended(&h);
        let running = start_prompt(&h, "hi");
        entered.notified().await;
        let before = roles(&h);
        release.send(true).unwrap();
        running.await.unwrap().unwrap();
        h.session.agent().wait_for_idle().await;
        ended.notified().await;
        assert_eq!(json!({"before": before, "after": roles(&h)}), expected);
    });
}

/// Copies the committed empty root `name` below `base`, returning the copy's path.
fn copy_empty_root(base: &str, name: &str) -> String {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    let copy = std::path::Path::new(base).join(name);
    std::fs::create_dir(&copy).unwrap();
    std::fs::copy(source.join(".gitkeep"), copy.join(".gitkeep")).unwrap();
    copy.to_str().unwrap().to_owned()
}

#[test]
fn empty_resource_roots_keep_the_session_isolated() {
    use maestro_app::{CreateAgentSessionOptions, ResourceLoaderOptions, SessionResources};
    let expected = expected("empty_roots");
    let test = "empty_resource_roots_keep_the_session_isolated";
    crate::isolated_home::with_isolated_home(test, |base| {
        let cwd = copy_empty_root(base, "empty-cwd");
        let agent_dir = copy_empty_root(base, "empty-agent");
        let counts = on_task_set(|local| async move {
            let h = harness(HarnessOptions::default()).await;
            let module_loader: maestro_extensions::ModuleLoader<Local> =
                Rc::new(|_| Box::pin(std::future::ready(Ok(None))));
            let resources = ResourceLoaderOptions::new(local, module_loader);
            let manager = maestro_session::SessionManager::in_memory(Some(&cwd)).unwrap();
            let settings =
                maestro_settings::SettingsManager::in_memory(maestro_settings::Settings::default());
            let created = maestro_app::create_agent_session(CreateAgentSessionOptions {
                cwd: Some(cwd.clone()),
                agent_dir: Some(agent_dir),
                auth_storage: None,
                model_registry: Some(Arc::clone(&h.registry)),
                model: Some(h.model("plain")),
                thinking_level: None,
                scoped_models: Vec::new(),
                no_tools: None,
                tools: None,
                custom_tools: Vec::new(),
                resources: SessionResources::Default(Box::new(resources)),
                session_manager: Some(Rc::new(RefCell::new(manager))),
                settings_manager: Some(Rc::new(RefCell::new(settings))),
                session_start_event: None,
                theme: maestro_theme::LiveTheme::default(),
                offline: None,
            })
            .await
            .unwrap();
            let loader = created.session.resource_loader();
            let counts = json!({
                "skills": loader.get_skills().skills.len(),
                "prompts": loader.get_prompts().prompts.len(),
                "extensions": loader.get_extensions().extensions.len(),
                "context_files": loader.get_agents_files().len(),
            });
            created.session.dispose();
            counts
        });
        assert_eq!(counts, expected);
    });
}
