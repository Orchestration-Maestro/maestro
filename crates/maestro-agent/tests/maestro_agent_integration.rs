//! Agent scenarios through the registered scripted model adapter.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support;
use maestro_agent::{AgentMessage, ThinkingLevel};
use maestro_models::{
    AssistantContent, FauxAssistantMessageOptions, FauxResponseStep, FauxToolCallOptions,
    RegisterFauxProviderOptions, StopReason, ToolResultMessage, UserContent,
    faux_assistant_message, faux_text, faux_thinking, faux_tool_call,
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex, RwLock};
use support::observable::kind;
use support::scripted::{
    Registered, agent_for, ask_calculator, block_text, history, last_assistant, record_events, say,
    set_history, streaming, user,
};
use support::utils::calculate::{calculate, calculate_tool};

/// A prompt answered by the registered adapter settles user then assistant history.
#[test]
fn maestro_scripted_answer_settles_state() {
    support::run_io(maestro_scripted_answer_settles_state_case());
}
/// Drive the scenario of `maestro_scripted_answer_settles_state` on the test runtime.
async fn maestro_scripted_answer_settles_state_case() {
    let mut registered = Registered::default();
    let faux = registered.add(RegisterFauxProviderOptions::default());
    faux.set_responses(vec![say("4")]);
    let agent = agent_for(faux.get_model(None).unwrap(), vec![]);
    agent
        .prompt(support::owned::input(
            "What is 2+2? Answer with just the number.",
        ))
        .await
        .unwrap();
    let entries = history(&agent);
    assert_eq!(
        entries.iter().map(|entry| entry.0).collect::<Vec<_>>(),
        ["user", "assistant"]
    );
    assert_eq!(entries[1].1, "4");
    assert!(!streaming(&agent));
}

/// A calculator call runs the real arithmetic and exposes its id while pending.
#[test]
fn maestro_calculator_execution_tracks_pending_ids() {
    support::run_io(maestro_calculator_execution_tracks_pending_ids_case());
}
/// Drive the scenario of `maestro_calculator_execution_tracks_pending_ids` on the test runtime.
async fn maestro_calculator_execution_tracks_pending_ids_case() {
    let mut registered = Registered::default();
    let faux = registered.add(RegisterFauxProviderOptions::default());
    faux.set_responses(vec![
        ask_calculator("calc-1", "123 * 456"),
        say("The result is 56088."),
    ]);
    let agent = agent_for(faux.get_model(None).unwrap(), vec![calculate_tool()]);
    let pending = Arc::new(Mutex::new(vec![]));
    let (sink, state) = (Arc::clone(&pending), Arc::clone(agent.state()));
    agent.subscribe(Arc::new(move |event, _| {
        let name = kind(&event.read().unwrap());
        if name.starts_with("tool_execution_") && name != "tool_execution_update" {
            let ids = state.read().unwrap().pending_tool_calls().to_vec();
            sink.lock().unwrap().push((name, ids));
        }
        Box::pin(std::future::ready(Ok(())))
    }));
    agent
        .prompt(support::owned::input(
            "Calculate 123 * 456 using the calculator tool.",
        ))
        .await
        .unwrap();
    let entries = history(&agent);
    let roles: Vec<_> = entries.iter().map(|entry| entry.0).collect();
    assert_eq!(roles, ["user", "assistant", "toolResult", "assistant"]);
    assert_eq!(entries[2].1, "123 * 456 = 56088");
    assert_eq!(entries[3].1, "The result is 56088.");
    let calc = vec!["calc-1".to_owned()];
    let expected = vec![
        ("tool_execution_start", calc),
        ("tool_execution_end", vec![]),
    ];
    assert_eq!(*pending.lock().unwrap(), expected);
    assert!(
        agent
            .state()
            .read()
            .unwrap()
            .pending_tool_calls()
            .is_empty()
    );
}

/// Aborting at the request boundary leaves an aborted response and clears streaming.
#[test]
fn maestro_stream_abort_clears_runtime_state() {
    support::run_io(maestro_stream_abort_clears_runtime_state_case());
}
/// Drive the scenario of `maestro_stream_abort_clears_runtime_state` on the test runtime.
async fn maestro_stream_abort_clears_runtime_state_case() {
    let mut registered = Registered::default();
    let options = RegisterFauxProviderOptions {
        tokens_per_second: Some(20.0),
        token_size: Some(maestro_models::FauxTokenSize {
            min: Some(2.0),
            max: Some(2.0),
        }),
        ..Default::default()
    };
    let faux = registered.add(options);
    let agent = agent_for(faux.get_model(None).unwrap(), vec![]);
    let aborting = Arc::clone(&agent);
    let slow = "one two three four five six seven eight nine ten eleven twelve";
    faux.set_responses(vec![FauxResponseStep::Factory(Arc::new(AbortingFactory {
        agent: aborting,
        text: slow,
    }))]);
    agent
        .prompt(support::owned::input("Count slowly from 1 to 20."))
        .await
        .unwrap();
    assert!(!streaming(&agent));
    let last = last_assistant(&agent);
    assert_eq!(last.stop_reason, StopReason::Aborted);
    assert!(last.error_message.is_some());
    assert_eq!(
        agent.state().read().unwrap().error_message(),
        last.error_message.as_deref()
    );
}

/// Chunked delivery yields the complete ordered event sequence and a two-entry history.
#[test]
fn maestro_chunked_response_emits_lifecycle() {
    support::run_io(maestro_chunked_response_emits_lifecycle_case());
}
/// Drive the scenario of `maestro_chunked_response_emits_lifecycle` on the test runtime.
async fn maestro_chunked_response_emits_lifecycle_case() {
    let mut registered = Registered::default();
    let options = RegisterFauxProviderOptions {
        token_size: Some(maestro_models::FauxTokenSize {
            min: Some(1.0),
            max: Some(1.0),
        }),
        ..Default::default()
    };
    let faux = registered.add(options);
    faux.set_responses(vec![say("1 2 3 4 5")]);
    let agent = agent_for(faux.get_model(None).unwrap(), vec![]);
    let events = record_events(&agent);
    agent
        .prompt(support::owned::input("Count from 1 to 5."))
        .await
        .unwrap();
    let mut expected = vec![
        "agent_start",
        "turn_start",
        "message_start",
        "message_end",
        "message_start",
    ];
    expected.extend(["message_update"; 5]);
    expected.extend(["message_end", "turn_end", "agent_end"]);
    assert_eq!(*events.lock().unwrap(), expected);
    assert_eq!(history(&agent).len(), 2);
    assert!(!streaming(&agent));
}

/// The second turn's request carries the first turn's user message.
#[test]
fn maestro_second_turn_receives_prior_context() {
    support::run_io(maestro_second_turn_receives_prior_context_case());
}
/// Drive the scenario of `maestro_second_turn_receives_prior_context` on the test runtime.
async fn maestro_second_turn_receives_prior_context_case() {
    let mut registered = Registered::default();
    let faux = registered.add(RegisterFauxProviderOptions::default());
    let remembers = FauxResponseStep::Factory(Arc::new(RememberingFactory));
    faux.set_responses(vec![say("Nice to meet you, Alice."), remembers]);
    let agent = agent_for(faux.get_model(None).unwrap(), vec![]);
    agent
        .prompt(support::owned::input("My name is Alice."))
        .await
        .unwrap();
    assert_eq!(history(&agent).len(), 2);
    agent
        .prompt(support::owned::input("What is my name?"))
        .await
        .unwrap();
    let entries = history(&agent);
    assert_eq!(entries.len(), 4);
    assert_eq!(entries[3], ("assistant", "Your name is Alice.".to_owned()));
}

/// Thinking and text blocks survive streaming in their original order.
#[test]
fn maestro_thinking_blocks_survive_streaming() {
    support::run_io(maestro_thinking_blocks_survive_streaming_case());
}
/// Drive the scenario of `maestro_thinking_blocks_survive_streaming` on the test runtime.
async fn maestro_thinking_blocks_survive_streaming_case() {
    let mut registered = Registered::default();
    let options = RegisterFauxProviderOptions {
        models: Some(vec![maestro_models::FauxModelDefinition {
            id: "faux-reasoning".into(),
            reasoning: Some(true),
            ..Default::default()
        }]),
        ..Default::default()
    };
    let faux = registered.add(options);
    let blocks = vec![
        AssistantContent::Thinking(faux_thinking("step by step")),
        AssistantContent::Text(faux_text("4")),
    ];
    faux.set_responses(vec![say(blocks)]);
    let agent = agent_for(faux.get_model(None).unwrap(), vec![]);
    agent.state().write().unwrap().thinking_level = ThinkingLevel::Low;
    agent
        .prompt(support::owned::input("What is 2+2?"))
        .await
        .unwrap();
    let content = serde_json::to_value(&last_assistant(&agent).content).unwrap();
    let expected =
        json!([{"type": "thinking", "thinking": "step by step"}, {"type": "text", "text": "4"}]);
    assert_eq!(content, expected);
}

/// Continuing an empty history is refused before any model request.
#[test]
fn maestro_empty_continue_reports_error() {
    support::run_io(maestro_empty_continue_reports_error_case());
}
/// Drive the scenario of `maestro_empty_continue_reports_error` on the test runtime.
async fn maestro_empty_continue_reports_error_case() {
    let mut registered = Registered::default();
    let faux = registered.add(RegisterFauxProviderOptions::default());
    let agent = agent_for(faux.get_model(None).unwrap(), vec![]);
    let error = agent.r#continue().await.unwrap_err();
    assert_eq!(error.message, "No messages to continue from");
    assert_eq!(faux.state.call_count(), 0);
}

/// Continuing from an assistant tail with nothing queued is refused before any model request.
#[test]
fn maestro_unqueued_assistant_tail_reports_error() {
    support::run_io(maestro_unqueued_assistant_tail_reports_error_case());
}
/// Drive the scenario of `maestro_unqueued_assistant_tail_reports_error` on the test runtime.
async fn maestro_unqueued_assistant_tail_reports_error_case() {
    let mut registered = Registered::default();
    let faux = registered.add(RegisterFauxProviderOptions::default());
    let agent = agent_for(faux.get_model(None).unwrap(), vec![]);
    set_history(
        &agent,
        vec![AgentMessage::Assistant(support::assistant("Hello"))],
    );
    let error = agent.r#continue().await.unwrap_err();
    assert_eq!(
        error.message,
        "Cannot continue from message role: assistant"
    );
    assert_eq!(faux.state.call_count(), 0);
}

/// Continuing from a user tail adds only the response.
#[test]
fn maestro_user_tail_can_continue() {
    support::run_io(maestro_user_tail_can_continue_case());
}
/// Drive the scenario of `maestro_user_tail_can_continue` on the test runtime.
async fn maestro_user_tail_can_continue_case() {
    let mut registered = Registered::default();
    let faux = registered.add(RegisterFauxProviderOptions::default());
    faux.set_responses(vec![say("HELLO WORLD")]);
    let agent = agent_for(faux.get_model(None).unwrap(), vec![]);
    set_history(&agent, vec![user("Say exactly: HELLO WORLD")]);
    agent.r#continue().await.unwrap();
    let entries = history(&agent);
    let roles: Vec<_> = entries.iter().map(|entry| entry.0).collect();
    assert_eq!(roles, ["user", "assistant"]);
    assert_eq!(entries[1].1, "HELLO WORLD");
    assert_eq!(faux.state.call_count(), 1);
    assert!(!streaming(&agent));
}

/// Continuing from a tool-result tail answers from the executed calculation.
#[test]
fn maestro_tool_result_tail_can_continue() {
    support::run_io(maestro_tool_result_tail_can_continue_case());
}
/// Drive the scenario of `maestro_tool_result_tail_can_continue` on the test runtime.
async fn maestro_tool_result_tail_can_continue_case() {
    let mut registered = Registered::default();
    let faux = registered.add(RegisterFauxProviderOptions::default());
    faux.set_responses(vec![say("The answer is 8.")]);
    let tool = calculate_tool();
    let execute = Arc::clone(&tool.read().unwrap().execute);
    let outcome = execute("calc-1".into(), json!({"expression": "5 + 3"}), None, None)
        .await
        .unwrap();
    let text = outcome
        .content
        .iter()
        .filter_map(block_text)
        .collect::<String>();
    assert_eq!(text, "5 + 3 = 8");
    let agent = agent_for(faux.get_model(None).unwrap(), vec![tool]);
    let call = AssistantContent::ToolCall(faux_tool_call(
        "calculate",
        json!({"expression": "5 + 3"}),
        FauxToolCallOptions {
            id: Some("calc-1".into()),
        },
    ));
    let options = FauxAssistantMessageOptions {
        stop_reason: Some(StopReason::ToolUse),
        ..Default::default()
    };
    let asked = faux_assistant_message(
        vec![
            AssistantContent::Text(faux_text("Let me calculate that.")),
            call,
        ],
        options,
    );
    let result = ToolResultMessage {
        tool_call_id: "calc-1".into(),
        tool_name: "calculate".into(),
        content: outcome.content,
        details: None,
        is_error: false,
        timestamp: 1.0,
        usage: None,
    };
    set_history(
        &agent,
        vec![
            user("What is 5 + 3?"),
            AgentMessage::Assistant(Arc::new(RwLock::new(asked))),
            AgentMessage::ToolResult(Arc::new(RwLock::new(result))),
        ],
    );
    agent.r#continue().await.unwrap();
    let entries = history(&agent);
    assert_eq!(entries.len(), 4);
    assert_eq!(entries[3], ("assistant", "The answer is 8.".to_owned()));
    assert!(!streaming(&agent));
}

/// The fixture does real arithmetic, reports a known result and fails on anything else.
#[test]
fn maestro_calculator_fixture_returns_known_results_and_errors() {
    for (expression, shown) in [("123 * 456", "123 * 456 = 56088"), ("5 + 3", "5 + 3 = 8")] {
        let result = calculate(expression).unwrap();
        let texts: Vec<_> = result.content.iter().filter_map(block_text).collect();
        assert_eq!(texts, [shown]);
        assert!(result.terminate.is_none());
    }
    for rejected in ["(", "2 - 1", "x + 1", "9223372036854775807 + 1"] {
        assert!(calculate(rejected).is_err(), "{rejected}");
    }
    let tool = calculate_tool();
    let tool = tool.read().unwrap();
    assert_eq!(
        (tool.label.as_str(), tool.definition.name.as_str()),
        ("Calculator", "calculate")
    );
    assert_eq!(
        tool.definition.parameters["required"],
        json!(["expression"])
    );
    assert_eq!(
        tool.definition.parameters["properties"]["expression"]["type"],
        "string"
    );
    let run = support::run(async {
        let ok = (tool.execute)("id".into(), json!({"expression": "5 + 3"}), None, None)
            .await
            .unwrap();
        let bad = (tool.execute)("id".into(), json!({"expression": "("}), None, None).await;
        let missing = (tool.execute)("id".into(), Value::Null, None, None).await;
        (ok.details, bad.is_err(), missing.is_err())
    });
    assert_eq!(run, (None, true, true));
}

/// Abort at the existing request boundary before publishing the scripted response.
struct AbortingFactory {
    agent: Arc<maestro_agent::Agent>,
    text: &'static str,
}
impl AbortingFactory {
    /// Preserve the same abort and response in either invocation family.
    fn respond<E: maestro_models::ProviderExecution>(
        &self,
        _: maestro_models::FauxResponseInput<E>,
    ) -> impl std::future::Future<
        Output = Result<maestro_models::AssistantMessage, maestro_models::DiagnosticErrorInfo>,
    >
    + 'static
    + use<E> {
        self.agent.abort();
        std::future::ready(Ok(faux_assistant_message(
            self.text,
            FauxAssistantMessageOptions::default(),
        )))
    }
}
support::scripted::factory!(AbortingFactory, |owner: &AbortingFactory, input| owner
    .respond(input));

/// Select the answer from the original request history.
struct RememberingFactory;
impl RememberingFactory {
    /// Inspect the original user entries without converting invocation options.
    fn respond<E: maestro_models::ProviderExecution>(
        input: &maestro_models::FauxResponseInput<E>,
    ) -> impl std::future::Future<
        Output = Result<maestro_models::AssistantMessage, maestro_models::DiagnosticErrorInfo>,
    >
    + 'static
    + use<E> {
        let alice = input.context.messages.iter().any(|message| match message {
            maestro_models::Message::User(user) => match &user.content {
                UserContent::Text(text) => text.contains("Alice"),
                UserContent::Blocks(blocks) => blocks
                    .iter()
                    .filter_map(block_text)
                    .any(|text| text.contains("Alice")),
            },
            _ => false,
        });
        let answer = if alice {
            "Your name is Alice."
        } else {
            "I do not know your name."
        };
        let message = faux_assistant_message(answer, FauxAssistantMessageOptions::default());
        std::future::ready(Ok(message))
    }
}
support::scripted::factory!(RememberingFactory, |_: &RememberingFactory, input| {
    RememberingFactory::respond(&input)
});
