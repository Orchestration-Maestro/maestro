//! Executable observations behind the Agent guide, run offline with controlled adapters.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support;
use futures_util::FutureExt;
use maestro_agent::{
    Agent, AgentContext, AgentEvent, AgentInitialState, AgentLoopOptions, AgentMessage,
    AgentOptions, AgentTool, AgentToolResult, CustomAgentMessages, PromptInput, QueueMode,
    ThinkingLevel, ToolExecutionMode,
};
use maestro_models::{
    AssistantMessageEvent, FauxAssistantMessageOptions, FauxResponseOptions, FauxResponseStep,
    Message, Model, RegisterFauxProviderOptions, TextContent, ThinkingBudgets, Tool, Transport,
    UserBlock, UserContent, faux_assistant_message,
};
use serde_json::json;
use std::sync::{Arc, Mutex, RwLock};
use support::observable::kind;
use support::scripted::{
    Registered, agent_for, block_text, history, last_assistant, record_events, say, spawner, user,
};

/// The shared handle of a registered model, as an Agent's initial model.
fn model_of(model: &Model) -> Arc<RwLock<Model>> {
    Arc::new(RwLock::new(model.clone()))
}
/// A ready, successful callback future.
fn ready<T: Send + 'static>(value: T) -> maestro_models::BoxFuture<T> {
    Box::pin(std::future::ready(value))
}

/// Install the quick-start Agent, subscribe to text deltas, prompt and settle without credentials.
#[test]
fn guide_quick_start_streams_offline() {
    support::run_io(guide_quick_start_streams_offline_case());
}
/// Drive the scenario of `guide_quick_start_streams_offline` on the test runtime.
async fn guide_quick_start_streams_offline_case() {
    let mut registered = Registered::default();
    let faux = registered.add(RegisterFauxProviderOptions {
        token_size: Some(maestro_models::FauxTokenSize {
            min: Some(1.0),
            max: Some(1.0),
        }),
        ..Default::default()
    });
    faux.set_responses(vec![say("Hello there, friend!")]);
    let agent = agent_for(faux.get_model(None).unwrap(), vec![]);
    let deltas = Arc::new(Mutex::new(vec![]));
    let sink = Arc::clone(&deltas);
    agent.subscribe(Arc::new(move |event, _| {
        if let AgentEvent::MessageUpdate {
            assistant_message_event: AssistantMessageEvent::TextDelta { delta, .. },
            ..
        } = &*event.read().unwrap()
        {
            sink.lock().unwrap().push(delta.clone());
        }
        ready(Ok(()))
    }));
    agent.prompt(support::owned::input("Hi")).await.unwrap();
    let deltas = deltas.lock().unwrap().clone();
    assert!(deltas.len() > 1, "{deltas:?}");
    assert_eq!(deltas.concat(), "Hello there, friend!");
    assert_eq!(
        history(&agent)[1],
        ("assistant", "Hello there, friend!".to_owned())
    );
    let inert: Arc<Agent> = Arc::new(Agent::default());
    let refused = inert.prompt(support::owned::input("Hi")).await.unwrap_err();
    assert_eq!(
        refused.message,
        "Agent requires a task spawner before prompting."
    );
}

/// A notice only the application understands.
struct Notice {
    /// Text shown to the user interface.
    text: String,
}
impl CustomAgentMessages for Notice {
    fn role(&self) -> &'static str {
        "notice"
    }
}

/// The text of every message in a model request, in order.
fn request_texts(messages: &[Message]) -> Vec<String> {
    let block = |blocks: &[UserBlock]| blocks.iter().filter_map(block_text).collect::<String>();
    messages
        .iter()
        .map(|message| match message {
            Message::User(user) => match &user.content {
                UserContent::Text(text) => text.clone(),
                UserContent::Blocks(blocks) => block(blocks),
            },
            Message::Assistant(_) => "assistant".to_owned(),
            Message::ToolResult(_) => "toolResult".to_owned(),
        })
        .collect()
}
/// Standard entries convert unchanged; a notice becomes a user message.
fn convert_notices() -> maestro_agent::ConvertToLlm<Notice> {
    Arc::new(|history| {
        let entries = history.read().unwrap();
        let messages = entries
            .iter()
            .map(|entry| match entry {
                AgentMessage::User(user) => Message::User(user.read().unwrap().clone()),
                AgentMessage::Assistant(reply) => Message::Assistant(reply.read().unwrap().clone()),
                AgentMessage::ToolResult(result) => {
                    Message::ToolResult(result.read().unwrap().clone())
                }
                AgentMessage::Custom(notice) => Message::User(maestro_models::UserMessage {
                    content: UserContent::Text(format!("Notice: {}", notice.read().unwrap().text)),
                    timestamp: 1.0,
                }),
            })
            .collect();
        ready(Ok(messages))
    })
}
/// Pruning runs before conversion, so the model sees the converted, pruned request only.
#[test]
fn guide_custom_messages_transform_before_conversion() {
    support::run_io(guide_custom_messages_transform_before_conversion_case());
}
/// Drive the scenario of `guide_custom_messages_transform_before_conversion` on the test runtime.
async fn guide_custom_messages_transform_before_conversion_case() {
    let mut registered = Registered::default();
    let faux = registered.add(RegisterFauxProviderOptions::default());
    let seen = Arc::new(Mutex::new(vec![]));
    let sink = Arc::clone(&seen);
    faux.set_responses(vec![FauxResponseStep::Factory(Arc::new(
        RequestTextFactory(sink),
    ))]);
    let notice = AgentMessage::Custom(Arc::new(RwLock::new(Notice {
        text: "Info".into(),
    })));
    let agent: Arc<Agent<Notice>> = Arc::new(Agent::new(AgentOptions {
        spawn: Some(spawner()),
        convert_to_llm: Some(convert_notices()),
        transform_context: Some(Arc::new(|history, _| {
            let kept: Vec<_> = history.read().unwrap().iter().skip(2).cloned().collect();
            ready(Ok(Arc::new(RwLock::new(kept))))
        })),
        initial_state: AgentInitialState {
            model: Some(model_of(faux.get_model(None).unwrap())),
            messages: vec![
                user("old question"),
                user("old answer"),
                notice,
                user("recent"),
            ],
            ..Default::default()
        },
        ..Default::default()
    }));
    let prompt = PromptInput::Text {
        text: "now".into(),
        images: vec![],
    };
    agent.prompt(prompt).await.unwrap();
    assert_eq!(*seen.lock().unwrap(), [["Notice: Info", "recent", "now"]]);
    let state = agent.state().read().unwrap();
    assert_eq!(state.messages().read().unwrap().len(), 6);
}

/// A fresh Agent shows exactly the documented defaults.
fn assert_documented_defaults() {
    let agent: Arc<Agent> = Arc::new(Agent::default());
    {
        let state = agent.state().read().unwrap();
        assert_eq!(state.system_prompt, "");
        assert_eq!(state.model.read().unwrap().id, "unknown");
        assert_eq!(state.thinking_level, ThinkingLevel::Off);
        assert!(state.tools().read().unwrap().is_empty());
        assert!(state.messages().read().unwrap().is_empty());
        assert!(!state.is_streaming() && state.streaming_message().is_none());
        assert!(state.pending_tool_calls().is_empty() && state.error_message().is_none());
    }
    let modes = (agent.steering_mode(), agent.follow_up_mode());
    assert_eq!(modes, (QueueMode::OneAtATime, QueueMode::OneAtATime));
    assert_eq!(
        *agent.tool_execution.read().unwrap(),
        ToolExecutionMode::Parallel
    );
    assert_eq!(*agent.transport.read().unwrap(), Transport::Auto);
    assert!(agent.session_id.read().unwrap().is_none());
    assert!(agent.max_retry_delay_ms.read().unwrap().is_none());
    assert!(agent.thinking_budgets.read().unwrap().is_none());
}
/// Run one prompt with constructor values only and check the original request options.
async fn request_options(budgets: &ThinkingBudgets) {
    let mut registered = Registered::default();
    let faux = registered.add(RegisterFauxProviderOptions::default());
    let seen = Arc::new(std::sync::atomic::AtomicBool::new(false));
    faux.set_responses(vec![FauxResponseStep::Factory(Arc::new(
        RequestOptionsFactory {
            seen: Arc::clone(&seen),
            budgets: budgets.clone(),
        },
    ))]);
    let configured: Arc<Agent> = Arc::new(Agent::new(AgentOptions {
        spawn: Some(spawner()),
        session_id: Some("session-123".into()),
        transport: Some(Transport::Websocket),
        max_retry_delay_ms: Some(1234.0),
        thinking_budgets: Some(Arc::new(RwLock::new(budgets.clone()))),
        get_api_key: Some(Arc::new(|_| ready(Ok(Some("resolved".into()))))),
        initial_state: AgentInitialState {
            model: Some(model_of(faux.get_model(None).unwrap())),
            thinking_level: Some(ThinkingLevel::Low),
            ..Default::default()
        },
        ..Default::default()
    }));
    configured
        .prompt(support::owned::input("Hi"))
        .await
        .unwrap();
    assert!(seen.load(std::sync::atomic::Ordering::SeqCst));
}

/// Defaults are exactly the documented ones, and constructor values reach the model request.
#[test]
fn guide_options_and_state_show_current_defaults() {
    support::run_io(guide_options_and_state_show_current_defaults_case());
}
/// Drive the scenario of `guide_options_and_state_show_current_defaults` on the test runtime.
async fn guide_options_and_state_show_current_defaults_case() {
    assert_documented_defaults();
    let budgets = ThinkingBudgets {
        minimal: Some(128.0),
        high: Some(2048.0),
        ..Default::default()
    };
    request_options(&budgets).await;
}

/// Arguments of the guide's echo tool.
#[derive(serde::Deserialize)]
struct Echo {
    /// Text to repeat.
    text: String,
}
/// The echo tool: renames a legacy field, reports progress and requests termination.
fn echo_tool() -> maestro_agent::SharedAgentTool {
    let definition = Tool {
        name: "echo".into(),
        description: "Repeat".into(),
        parameters: json!({
            "type": "object",
            "properties": {"text": {"type": "string"}},
            "required": ["text"]
        })
        .into(),
    };
    let text = |text: String| UserBlock::Text(TextContent::new(text));
    let mut tool = AgentTool::typed(
        definition,
        "Echo".into(),
        Arc::new(move |_, params: Echo, _, progress| {
            if let Some(progress) = progress {
                progress(AgentToolResult {
                    content: vec![text("working".into())],
                    details: Some(json!({"stage": 1})),
                    terminate: None,
                });
            }
            ready(Ok(AgentToolResult {
                content: vec![text(params.text)],
                details: Some(json!({"audited": false})),
                terminate: Some(true),
            }))
        }),
    );
    tool.prepare_arguments = Some(Arc::new(|raw| {
        if let Some(message) = raw.get("message").cloned() {
            raw["text"] = message;
        }
        Ok(raw.clone())
    }));
    Arc::new(RwLock::new(tool))
}
/// An assistant turn calling `echo` twice with the legacy `message` field.
fn two_echo_calls() -> FauxResponseStep {
    let call = |id: &str, text: &str| {
        let options = maestro_models::FauxToolCallOptions {
            id: Some(id.into()),
        };
        let call = maestro_models::faux_tool_call("echo", json!({"message": text}), options);
        maestro_models::AssistantContent::ToolCall(call)
    };
    let options = FauxAssistantMessageOptions {
        stop_reason: Some(maestro_models::StopReason::ToolUse),
        ..Default::default()
    };
    let content = vec![call("a", "one"), call("b", "two")];
    FauxResponseStep::Message(Box::new(faux_assistant_message(content, options)))
}
/// An Agent whose hooks edit arguments before and details after execution.
fn agent_with_hooks(model: &Model, tools: Vec<maestro_agent::SharedAgentTool>) -> Arc<Agent> {
    let edit_arguments: maestro_agent::BeforeToolCall<maestro_agent::NoCustomMessages> =
        Arc::new(|context, _| {
            context.args["text"] = json!("EDITED");
            Box::pin(std::future::ready(Ok(None)))
        });
    let audit: maestro_agent::AfterToolCall<maestro_agent::NoCustomMessages> = Arc::new(|_, _| {
        let replacement = maestro_agent::AfterToolCallResult {
            details: Some(json!({"audited": true})),
            ..Default::default()
        };
        Box::pin(std::future::ready(Ok(Some(replacement))))
    });
    Arc::new(Agent::new(AgentOptions {
        spawn: Some(spawner()),
        before_tool_call: Some(edit_arguments),
        after_tool_call: Some(audit),
        initial_state: AgentInitialState {
            model: Some(model_of(model)),
            tools,
            ..Default::default()
        },
        ..Default::default()
    }))
}
/// Record the tool events of an Agent as short descriptions.
fn record_tool_events(agent: &Arc<Agent>) -> Arc<Mutex<Vec<String>>> {
    let seen = Arc::new(Mutex::new(vec![]));
    let sink = Arc::clone(&seen);
    agent.subscribe(Arc::new(move |event, _| {
        let note = match &*event.read().unwrap() {
            AgentEvent::ToolExecutionStart { tool_call } => Some(format!(
                "start {} {}",
                tool_call.id, tool_call.arguments["message"]
            )),
            AgentEvent::ToolExecutionUpdate {
                tool_call,
                partial_result,
            } => Some(format!(
                "update {} {}",
                tool_call.id,
                json!(partial_result.details)
            )),
            AgentEvent::ToolExecutionEnd {
                tool_call,
                result,
                is_error,
            } => Some(format!(
                "end {} {} {is_error}",
                tool_call.id,
                json!(result.details)
            )),
            _ => None,
        };
        sink.lock().unwrap().extend(note);
        ready(Ok(()))
    }));
    seen
}

/// Tool progress, hooks, failures and unanimous termination as the guide describes them.
#[test]
fn guide_tools_hooks_progress_and_termination() {
    support::run_io(guide_tools_hooks_progress_and_termination_case());
}
/// Drive the scenario of `guide_tools_hooks_progress_and_termination` on the test runtime.
async fn guide_tools_hooks_progress_and_termination_case() {
    let mut registered = Registered::default();
    let faux = registered.add(RegisterFauxProviderOptions::default());
    faux.set_responses(vec![two_echo_calls(), say("unused")]);
    let agent = agent_with_hooks(faux.get_model(None).unwrap(), vec![echo_tool()]);
    let observed = record_tool_events(&agent);
    agent
        .prompt(support::owned::input("Run both"))
        .await
        .unwrap();
    let observed = observed.lock().unwrap().clone();
    for expected in [
        r#"start a "one""#,
        r#"update b {"stage":1}"#,
        r#"end a {"audited":true} false"#,
    ] {
        assert!(
            observed.contains(&expected.to_owned()),
            "{expected}: {observed:?}"
        );
    }
    let entries = history(&agent);
    let roles: Vec<_> = entries.iter().map(|entry| entry.0).collect();
    assert_eq!(roles, ["user", "assistant", "toolResult", "toolResult"]);
    assert_eq!(
        (entries[2].1.as_str(), entries[3].1.as_str()),
        ("EDITED", "EDITED")
    );
    assert_eq!(faux.state.call_count(), 1);
    assert_eq!(faux.get_pending_response_count(), 1);
}

/// Arguments of the guide's flaky tool; `count` is required by the type but not by the schema.
#[derive(serde::Deserialize)]
struct Flaky {
    /// Text deciding whether the callback fails.
    text: String,
    /// Required by decoding only.
    count: u8,
}
/// A tool whose callback fails for `boom`, counting how often it runs.
fn flaky_tool(runs: &Arc<Mutex<u32>>) -> maestro_agent::SharedAgentTool {
    let runs = Arc::clone(runs);
    let definition = Tool {
        name: "flaky".into(),
        description: "Fail on demand".into(),
        parameters: json!({"type": "object", "properties": {"text": {"type": "string"}}}).into(),
    };
    let tool = AgentTool::typed(
        definition,
        "Flaky".into(),
        Arc::new(move |_, params: Flaky, _, _| {
            *runs.lock().unwrap() += u32::from(params.count);
            let outcome = if params.text == "boom" {
                Err(maestro_models::extract_diagnostic_error(
                    maestro_models::DiagnosticInput::Text("boom failed"),
                ))
            } else {
                Ok(AgentToolResult {
                    content: vec![],
                    details: Some(json!({})),
                    terminate: None,
                })
            };
            ready(outcome)
        }),
    );
    Arc::new(RwLock::new(tool))
}

/// A callback `Err` and an argument decoding failure both become error results without a second run.
#[test]
fn guide_tool_failures_become_error_results() {
    support::run_io(guide_tool_failures_become_error_results_case());
}
/// Drive the scenario of `guide_tool_failures_become_error_results` on the test runtime.
async fn guide_tool_failures_become_error_results_case() {
    let mut registered = Registered::default();
    let faux = registered.add(RegisterFauxProviderOptions::default());
    let call = |id: &str, arguments: serde_json::Value| {
        let options = maestro_models::FauxToolCallOptions {
            id: Some(id.into()),
        };
        maestro_models::AssistantContent::ToolCall(maestro_models::faux_tool_call(
            "flaky", arguments, options,
        ))
    };
    let options = FauxAssistantMessageOptions {
        stop_reason: Some(maestro_models::StopReason::ToolUse),
        ..Default::default()
    };
    let content = vec![
        call("a", json!({"text": "boom", "count": 1})),
        call("b", json!({"text": "fine"})),
    ];
    let calls = FauxResponseStep::Message(Box::new(faux_assistant_message(content, options)));
    faux.set_responses(vec![calls, say("recovered")]);
    let runs = Arc::new(Mutex::new(0));
    let agent = agent_for(faux.get_model(None).unwrap(), vec![flaky_tool(&runs)]);
    let ends = Arc::new(Mutex::new(vec![]));
    let sink = Arc::clone(&ends);
    agent.subscribe(Arc::new(move |event, _| {
        if let AgentEvent::ToolExecutionEnd {
            tool_call,
            result,
            is_error,
        } = &*event.read().unwrap()
        {
            let text = block_text(&result.content[0]).unwrap().to_owned();
            sink.lock()
                .unwrap()
                .push((tool_call.id.clone(), text, *is_error));
        }
        ready(Ok(()))
    }));
    agent.prompt(support::owned::input("Go")).await.unwrap();
    assert_eq!(*runs.lock().unwrap(), 1);
    let ends = ends.lock().unwrap().clone();
    assert_eq!(ends.len(), 2);
    assert_eq!(ends[0], ("a".to_owned(), "boom failed".to_owned(), true));
    assert_eq!((ends[1].0.as_str(), ends[1].2), ("b", true));
    assert!(ends[1].1.contains("count"), "{}", ends[1].1);
    let entries = history(&agent);
    let roles: Vec<_> = entries.iter().map(|entry| entry.0).collect();
    assert_eq!(
        roles,
        ["user", "assistant", "toolResult", "toolResult", "assistant"]
    );
    assert_eq!(entries[4].1, "recovered");
}

/// Final history texts of a prompt run with the given queue policies and queued input.
async fn queued_run(steering: QueueMode, follow_up: QueueMode) -> Vec<String> {
    let mut registered = Registered::default();
    let faux = registered.add(RegisterFauxProviderOptions::default());
    faux.set_responses(vec![say("r1"), say("r2"), say("r3")]);
    let agent = agent_for(faux.get_model(None).unwrap(), vec![]);
    agent.set_steering_mode(steering);
    agent.set_follow_up_mode(follow_up);
    for text in ["s1", "s2"] {
        agent.steer(user(text));
    }
    for text in ["f1", "f2"] {
        agent.follow_up(user(text));
    }
    agent.prompt(support::owned::input("P")).await.unwrap();
    history(&agent).into_iter().map(|entry| entry.1).collect()
}

/// An awaited `AgentEnd` listener holds settlement, and removal stops later notifications.
async fn settle_and_unsubscribe() {
    let mut registered = Registered::default();
    let faux = registered.add(RegisterFauxProviderOptions::default());
    faux.set_responses(vec![say("one"), say("two")]);
    let agent = agent_for(faux.get_model(None).unwrap(), vec![]);
    let (entered, release) = (
        Arc::new(tokio::sync::Notify::new()),
        Arc::new(tokio::sync::Notify::new()),
    );
    let (hold_entered, hold_release) = (Arc::clone(&entered), Arc::clone(&release));
    agent.subscribe(Arc::new(move |event, _| {
        let ending = matches!(&*event.read().unwrap(), AgentEvent::AgentEnd { .. });
        let (entered, release) = (Arc::clone(&hold_entered), Arc::clone(&hold_release));
        Box::pin(async move {
            if ending {
                entered.notify_one();
                release.notified().await;
            }
            Ok(())
        })
    }));
    let counted = Arc::new(Mutex::new(0));
    let tally = Arc::clone(&counted);
    let subscription = agent.subscribe(Arc::new(move |_, _| {
        *tally.lock().unwrap() += 1;
        Box::pin(std::future::ready(Ok(())))
    }));
    let run = agent.prompt(support::owned::input("first"));
    entered.notified().await;
    assert!(agent.wait_for_idle().now_or_never().is_none());
    assert!(support::scripted::streaming(&agent));
    release.notify_one();
    run.await.unwrap();
    assert!(agent.wait_for_idle().now_or_never().is_some());
    let first = *counted.lock().unwrap();
    assert!(first > 0);
    subscription.unsubscribe();
    let run = agent.prompt(support::owned::input("second"));
    entered.notified().await;
    release.notify_one();
    run.await.unwrap();
    assert_eq!(*counted.lock().unwrap(), first);
}

/// Message-start events a continuation emits from the given history tail.
async fn continuation_starts(tail: AgentMessage, queued: Option<&str>) -> (Vec<String>, usize) {
    let mut registered = Registered::default();
    let faux = registered.add(RegisterFauxProviderOptions::default());
    faux.set_responses(vec![say("more")]);
    let agent = agent_for(faux.get_model(None).unwrap(), vec![]);
    support::scripted::set_history(&agent, vec![tail]);
    if let Some(text) = queued {
        agent.follow_up(user(text));
    }
    let events = record_events(&agent);
    agent.r#continue().await.unwrap();
    let starts = events
        .lock()
        .unwrap()
        .iter()
        .filter(|name| **name == "message_start")
        .count();
    assert_eq!(
        last_assistant(&agent).stop_reason,
        maestro_models::StopReason::Stop
    );
    (
        history(&agent).into_iter().map(|entry| entry.1).collect(),
        starts,
    )
}

/// An assistant tail needs queued input to continue; other tails add no events for old entries.
async fn continue_from_assistant_tail() {
    let mut registered = Registered::default();
    let faux = registered.add(RegisterFauxProviderOptions::default());
    let agent = agent_for(faux.get_model(None).unwrap(), vec![]);
    support::scripted::set_history(
        &agent,
        vec![AgentMessage::Assistant(support::assistant("earlier"))],
    );
    let refused = agent.r#continue().await.unwrap_err();
    assert_eq!(
        refused.message,
        "Cannot continue from message role: assistant"
    );
    let assistant = || AgentMessage::Assistant(support::assistant("earlier"));
    let (texts, starts) = continuation_starts(assistant(), Some("queued")).await;
    assert_eq!(
        (texts, starts),
        (["earlier", "queued", "more"].map(String::from).to_vec(), 2)
    );
    let (texts, starts) = continuation_starts(user("question"), None).await;
    assert_eq!(
        (texts, starts),
        (["question", "more"].map(String::from).to_vec(), 1)
    );
}

/// Queue policies drain independently; listeners are awaited; assistant-tail continuation works.
#[test]
fn guide_queues_and_subscriptions_settle() {
    support::run_io(guide_queues_and_subscriptions_settle_case());
}
/// Drive the scenario of `guide_queues_and_subscriptions_settle` on the test runtime.
async fn guide_queues_and_subscriptions_settle_case() {
    let all_steering = queued_run(QueueMode::All, QueueMode::OneAtATime).await;
    assert_eq!(
        all_steering,
        ["P", "s1", "s2", "r1", "f1", "r2", "f2", "r3"]
    );
    let all_follow_up = queued_run(QueueMode::OneAtATime, QueueMode::All).await;
    assert_eq!(
        all_follow_up,
        ["P", "s1", "r1", "s2", "r2", "f1", "f2", "r3"]
    );
    settle_and_unsubscribe().await;
    continue_from_assistant_tail().await;
}

/// Labels of every event a raw loop reports to its sink.
type Labels = Arc<Mutex<Vec<&'static str>>>;
/// A sink recording event labels.
fn label_sink() -> (maestro_agent::AgentEventSink, Labels) {
    let labels = Labels::default();
    let sink = Arc::clone(&labels);
    let emit: maestro_agent::AgentEventSink = Arc::new(move |event| {
        sink.lock().unwrap().push(kind(&event));
        Box::pin(std::future::ready(Ok(())))
    });
    (emit, labels)
}
/// Label sequence of one prompt-shaped run.
const PROMPT_RUN: [&str; 8] = [
    "agent_start",
    "turn_start",
    "message_start",
    "message_end",
    "message_start",
    "message_end",
    "turn_end",
    "agent_end",
];

/// The awaited loops report to an awaited sink and return the generated entries.
async fn awaited_loops() {
    let config = support::config();
    let (emit, labels) = label_sink();
    let all = maestro_agent::run_agent_loop(
        vec![support::user("p")],
        support::context(),
        &config,
        emit,
        support::responses(vec![support::assistant("a1")]),
    )
    .await
    .unwrap();
    assert_eq!(all.len(), 2);
    let names = labels.lock().unwrap().clone();
    assert_eq!(names, PROMPT_RUN);
    let (emit, labels) = label_sink();
    let generated = maestro_agent::run_agent_loop_continue(
        support::context(),
        &config,
        emit,
        support::responses(vec![support::assistant("a2")]),
    )
    .await
    .unwrap();
    assert_eq!(generated.len(), 1);
    assert_eq!(
        labels
            .lock()
            .unwrap()
            .iter()
            .filter(|name| **name == "message_start")
            .count(),
        1
    );
}

/// The observable loops return a stream whose result does not wait for event readers.
async fn observable_loops() {
    let config = Arc::new(support::config());
    let tasks = Arc::new(support::owned::Tasks::default());
    let options = support::responses(vec![support::assistant("a3")]);
    let stream = maestro_agent::agent_loop(
        vec![support::user("p")],
        support::context(),
        Arc::clone(&config),
        options,
        tasks.spawner(),
    );
    tasks.take().await;
    assert_eq!(stream.result().await.len(), 2);
    let mut names = vec![];
    while let Some(event) = stream.next().await {
        names.push(kind(&event));
    }
    assert_eq!(names, PROMPT_RUN);
    let empty = AgentContext {
        messages: Arc::new(RwLock::new(vec![])),
        ..support::context()
    };
    let refused = maestro_agent::agent_loop_continue(
        empty,
        Arc::clone(&config),
        AgentLoopOptions::default(),
        tasks.spawner(),
    );
    assert_eq!(
        refused.err().unwrap().message,
        "Cannot continue: no messages in context"
    );
    let options = support::responses(vec![support::assistant("a4")]);
    let stream =
        maestro_agent::agent_loop_continue(support::context(), config, options, tasks.spawner())
            .unwrap();
    tasks.take().await;
    assert_eq!(stream.result().await.len(), 1);
}

/// A proxy-backed Agent settles from a controlled transport, and all four raw loops run offline.
#[test]
fn guide_proxy_and_raw_loops_run_offline() {
    support::run_io(guide_proxy_and_raw_loops_run_offline_case());
}
/// Drive the scenario of `guide_proxy_and_raw_loops_run_offline` on the test runtime.
async fn guide_proxy_and_raw_loops_run_offline_case() {
    let (fetch, requests) = support::proxy::replying(support::proxy::Reply::chunks(
        support::proxy::canonical_lines(),
    ));
    let agent = support::proxy::proxy_agent(support::proxy::proxy_stream_fn(
        "https://proxy.example".into(),
        Some(fetch),
    ));
    agent.prompt(support::owned::input("Hi")).await.unwrap();
    assert_eq!(history(&agent)[1], ("assistant", "hé🙂!".to_owned()));
    assert_eq!(requests.lock().unwrap().len(), 1);
    assert_eq!(
        requests.lock().unwrap()[0].url,
        "https://proxy.example/api/stream"
    );
    awaited_loops().await;
    observable_loops().await;
}

/// Observe the converted and pruned conversation at the scripted provider.
struct RequestTextFactory(Arc<Mutex<Vec<Vec<String>>>>);
impl RequestTextFactory {
    /// Record the exact request text before selecting the response.
    fn respond<E: maestro_models::ProviderExecution>(
        &self,
        input: &maestro_models::FauxResponseInput<E>,
    ) -> impl std::future::Future<
        Output = Result<maestro_models::AssistantMessage, maestro_models::DiagnosticErrorInfo>,
    >
    + 'static
    + use<E> {
        let texts = request_texts(&input.context.messages);
        self.0.lock().unwrap().push(texts);
        std::future::ready(Ok(faux_assistant_message(
            "ok",
            FauxAssistantMessageOptions::default(),
        )))
    }
}
support::scripted::factory!(RequestTextFactory, |owner: &RequestTextFactory, input| {
    owner.respond(&input)
});

/// Observe constructor configuration at the consuming factory, before any projection.
struct RequestOptionsFactory {
    seen: Arc<std::sync::atomic::AtomicBool>,
    budgets: ThinkingBudgets,
}
impl RequestOptionsFactory {
    /// Check each documented option in the original simple record.
    fn respond<E: maestro_models::ProviderExecution>(
        &self,
        input: maestro_models::FauxResponseInput<E>,
    ) -> impl std::future::Future<
        Output = Result<maestro_models::AssistantMessage, maestro_models::DiagnosticErrorInfo>,
    >
    + 'static
    + use<E> {
        let Some(FauxResponseOptions::Simple(options)) = input.options else {
            panic!("the Agent requests through the simple adapter");
        };
        assert_eq!(options.common.session_id.as_deref(), Some("session-123"));
        assert_eq!(options.common.transport, Some(Transport::Websocket));
        assert_eq!(options.common.max_retry_delay_ms, Some(1234.0));
        assert_eq!(options.common.api_key.as_deref(), Some("resolved"));
        assert_eq!(options.reasoning, Some(maestro_models::ThinkingLevel::Low));
        assert_eq!(options.thinking_budgets, Some(self.budgets.clone()));
        self.seen.store(true, std::sync::atomic::Ordering::SeqCst);
        std::future::ready(Ok(faux_assistant_message(
            "ok",
            FauxAssistantMessageOptions::default(),
        )))
    }
}
support::scripted::factory!(
    RequestOptionsFactory,
    |owner: &RequestOptionsFactory, input| owner.respond(input)
);
