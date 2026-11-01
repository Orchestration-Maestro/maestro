//! Observable raw conversation scenarios.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support;
use maestro_agent::{
    AgentContext, AgentEvent, AgentLoopConfig, AgentMessage, CustomAgentMessages,
    ToolExecutionMode, agent_loop_continue,
};
use maestro_models::{Message, UserContent};
use std::sync::{
    Arc, Mutex, RwLock,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use support::{observable::*, *};

/// Lifecycle and result retain the actual prompt and response handles.
#[test]
fn maestro_raw_loop_returns_new_messages() {
    let context = context();
    context.messages.write().unwrap().clear();
    let prompt = user("Hello");
    let response = assistant("Hi there!");
    let recorded = observe(
        vec![prompt.clone()],
        context,
        config(),
        responses(vec![Arc::clone(&response)]),
    );
    assert_eq!(
        recorded.events.iter().map(kind).collect::<Vec<_>>(),
        [
            "agent_start",
            "turn_start",
            "message_start",
            "message_end",
            "message_start",
            "message_end",
            "turn_end",
            "agent_end"
        ]
    );
    assert_eq!(
        recorded.messages.iter().map(text).collect::<Vec<_>>(),
        ["Hello", "Hi there!"]
    );
    let (AgentMessage::User(input), AgentMessage::User(output)) = (&prompt, &recorded.messages[0])
    else {
        panic!("user missing")
    };
    assert!(Arc::ptr_eq(input, output));
    let AgentMessage::Assistant(output) = &recorded.messages[1] else {
        panic!("response missing")
    };
    assert!(Arc::ptr_eq(&response, output));
}

/// Custom entries project only at the caller's conversion boundary.
#[test]
fn maestro_custom_roles_are_projected_by_caller() {
    let (context, mut config) = notice_context();
    let history = Arc::clone(&context.messages);
    config.convert_to_llm = Arc::new(|entries| {
        assert_eq!(entries.read().unwrap().len(), 2);
        let messages = entries
            .read()
            .unwrap()
            .iter()
            .filter_map(|entry| match entry {
                AgentMessage::User(value) => Some(Message::User(value.read().unwrap().clone())),
                _ => None,
            })
            .collect();
        ready(Ok(messages))
    });
    let options = maestro_agent::AgentLoopOptions {
        stream_fn: Some(Arc::new(|_, request, _, _| {
            assert_eq!(request.messages.len(), 1);
            let Message::User(value) = &request.messages[0] else {
                panic!("expected user")
            };
            assert_eq!(value.content, UserContent::Text("Hello".into()));
            ready(Ok(model_stream(assistant("Hi there!"))))
        })),
        signal: None,
    };
    let prompt = AgentMessage::User(match user("Hello") {
        AgentMessage::User(value) => value,
        _ => unreachable!(),
    });
    let recorded = observe(vec![prompt], context, config, options);
    assert_eq!(recorded.messages.len(), 2);
    assert!(matches!(
        history.read().unwrap()[0],
        AgentMessage::Custom(_)
    ));
    assert_eq!(history.read().unwrap().len(), 1);
}

/// Conversion receives the selected transformed entries in their actual order.
#[test]
fn maestro_transform_precedes_model_conversion() {
    let context = context();
    *context.messages.write().unwrap() = vec![
        user("old message 1"),
        AgentMessage::Assistant(assistant("old response 1")),
        user("old message 2"),
        AgentMessage::Assistant(assistant("old response 2")),
    ];
    let mut config = config();
    config.transform_context = Some(Arc::new(|entries, _| {
        let entries = entries.read().unwrap();
        ready(Ok(Arc::new(RwLock::new(
            entries[entries.len() - 2..].to_vec(),
        ))))
    }));
    let convert = Arc::clone(&config.convert_to_llm);
    config.convert_to_llm = Arc::new(move |entries| {
        assert_eq!(
            entries.read().unwrap().iter().map(text).collect::<Vec<_>>(),
            ["old response 2", "new message"]
        );
        convert(entries)
    });
    let recorded = observe(
        vec![user("new message")],
        context,
        config,
        responses(vec![assistant("done")]),
    );
    assert_eq!(
        recorded.messages.iter().map(text).collect::<Vec<_>>(),
        ["new message", "done"]
    );
}

/// Echo execution, event payloads and final artifacts agree.
#[test]
fn maestro_tool_turn_returns_tool_results() {
    let echo = tool(
        "echo",
        Arc::new(|id, args, _, _| {
            assert_eq!(id, "tool-1");
            assert_eq!(args["value"], "hello");
            ready(Ok(artifact("hello", false)))
        }),
    );
    let recorded = observe(
        vec![user("Hello")],
        tools(vec![echo]),
        config(),
        responses(vec![
            calls(&["echo"], &serde_json::json!({"value":"hello"})),
            assistant("final"),
        ]),
    );
    assert_eq!(recorded.messages.len(), 4);
    assert_eq!(text(&recorded.messages[2]), "hello");
    assert_eq!(text(&recorded.messages[3]), "final");
    let ids = recorded
        .events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolExecutionStart { tool_call } => Some(tool_call.id.as_str()),
            AgentEvent::ToolExecutionEnd {
                tool_call,
                result,
                is_error,
            } => {
                assert!(!is_error);
                assert_eq!(result.details, Some(serde_json::json!({"output":"hello"})));
                Some(tool_call.id.as_str())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(ids, ["tool-1", "tool-1"]);
}

/// Before hooks edit validated arguments without a second validation pass.
#[test]
fn maestro_before_hook_mutates_validated_arguments() {
    let echo = tool(
        "echo",
        Arc::new(|_, args, _, _| {
            assert_eq!(args["value"], 123);
            ready(Ok(artifact("numeric", true)))
        }),
    );
    echo.write().unwrap().definition.parameters = serde_json::json!({"type":"object","properties":{"value":{"type":"string"}},"required":["value"]}).into();
    let mut config = config();
    config.before_tool_call = Some(Arc::new(|call, _| {
        assert_eq!(call.args["value"], "hello");
        call.args["value"] = serde_json::json!(123);
        ready(Ok(None))
    }));
    let recorded = observe(
        vec![],
        tools(vec![echo]),
        config,
        responses(vec![calls(
            &["echo"],
            &serde_json::json!({"value":"hello"}),
        )]),
    );
    assert_eq!(text(&recorded.messages[1]), "numeric");
}

/// Preparation reshapes flat edit input before schema validation.
#[test]
fn maestro_preparation_runs_before_validation() {
    let echo = tool(
        "edit",
        Arc::new(|_, args, _, _| {
            assert_eq!(
                args,
                serde_json::json!({"edits":[{"oldText":"before","newText":"after"}]})
            );
            ready(Ok(artifact("edited", true)))
        }),
    );
    {
        let mut echo = echo.write().unwrap();
        echo.definition.parameters = serde_json::json!({"type":"object","properties":{"edits":{"type":"array","items":{"type":"object","properties":{"oldText":{"type":"string"},"newText":{"type":"string"}},"required":["oldText","newText"]}}},"required":["edits"]}).into();
        echo.prepare_arguments = Some(Arc::new(|args| {
            Ok(serde_json::json!({"edits":[{"oldText":args["oldText"],"newText":args["newText"]}]}))
        }));
    }
    let mut config = config();
    config.should_stop_after_turn = Some(Arc::new(|_| ready(Ok(true))));
    let recorded = observe(
        vec![],
        tools(vec![echo]),
        config,
        responses(vec![calls(
            &["edit"],
            &serde_json::json!({"oldText":"before","newText":"after"}),
        )]),
    );
    assert_eq!(text(&recorded.messages[1]), "edited");
}

/// Preparation mutations stay in history even when the callback fails.
#[test]
fn preparation_mutation_survives_callback_failure() {
    let echo = tool(
        "echo",
        Arc::new(|_, _, _, _| unreachable!("failed preparation executed")),
    );
    echo.write().unwrap().prepare_arguments = Some(Arc::new(|args| {
        args["value"] = serde_json::json!(8);
        Err(maestro_models::DiagnosticErrorInfo::error("prepare failed"))
    }));
    let response = calls(&["echo"], &serde_json::json!({"value":"raw"}));
    let recorded = observe(
        vec![],
        tools(vec![echo]),
        config(),
        responses(vec![Arc::clone(&response), assistant("done")]),
    );
    let maestro_models::AssistantContent::ToolCall(call) = &response.read().unwrap().content[0]
    else {
        panic!("missing call");
    };
    assert_eq!(call.arguments["value"], 8);
    assert_eq!(text(&recorded.messages[1]), "prepare failed");
}

/// Completion order differs from artifact and turn-result source order.
#[test]
fn maestro_tool_events_follow_completion_results_follow_source() {
    let (release, gate) = tokio::sync::oneshot::channel();
    let gate = Arc::new(Mutex::new(Some(gate)));
    let echo = tool(
        "echo",
        Arc::new(move |id, _, _, _| {
            let wait = if id == "tool-1" {
                Some(gate.lock().unwrap().take().unwrap())
            } else {
                None
            };
            Box::pin(async move {
                if let Some(wait) = wait {
                    wait.await.unwrap();
                }
                Ok(artifact(&id, true))
            })
        }),
    );
    let mut config = config();
    config.tool_execution = None;
    let tasks = Arc::new(owned::Tasks::default());
    let stream = maestro_agent::agent_loop(
        vec![],
        tools(vec![echo]),
        Arc::new(config),
        responses(vec![calls(&["echo", "echo"], &serde_json::json!({}))]),
        tasks.spawner(),
    );
    let mut task = tasks.take();
    assert!(poll(task.as_mut()).is_pending());
    release.send(()).unwrap();
    run(task);
    let recorded = drain(&stream);
    assert_eq!(
        recorded
            .events
            .iter()
            .filter_map(|e| match e {
                AgentEvent::ToolExecutionEnd { tool_call, .. } => Some(tool_call.id.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>(),
        ["tool-2", "tool-1"]
    );
    assert_eq!(
        recorded.messages[1..].iter().map(text).collect::<Vec<_>>(),
        ["tool-1", "tool-2"]
    );
    assert_source_results(&recorded.events);
}

/// Steering available after the first execution waits for the batch artifacts.
#[test]
fn maestro_steering_waits_for_all_tools() {
    let executed = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&executed);
    let echo = tool(
        "echo",
        Arc::new(move |id, _, _, _| {
            count.fetch_add(1, Ordering::SeqCst);
            ready(Ok(artifact(&id, false)))
        }),
    );
    let mut config = config();
    config.tool_execution = Some(ToolExecutionMode::Sequential);
    let delivered = AtomicBool::new(false);
    config.get_steering_messages = Some(Arc::new(move || {
        let n = executed.load(Ordering::SeqCst);
        ready(Ok(if n >= 1 && !delivered.swap(true, Ordering::SeqCst) {
            vec![user("interrupt")]
        } else {
            vec![]
        }))
    }));
    let options = responses(vec![
        calls(&["echo", "echo"], &serde_json::json!({})),
        assistant("final"),
    ]);
    let source = options.stream_fn.unwrap();
    let requests = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&requests);
    let options = maestro_agent::AgentLoopOptions {
        signal: None,
        stream_fn: Some(Arc::new(move |model, request, settings, extra| {
            if count.fetch_add(1, Ordering::SeqCst) == 1 {
                assert_eq!(request.messages.len(), 5);
                let Message::User(last) = &request.messages[4] else {
                    panic!("interrupt missing")
                };
                assert_eq!(last.content, UserContent::Text("interrupt".into()));
                assert!(matches!(request.messages[2], Message::ToolResult(_)));
                assert!(matches!(request.messages[3], Message::ToolResult(_)));
            }
            source(model, request, settings, extra)
        })),
    };
    let recorded = observe(vec![], tools(vec![echo]), config, options);
    assert_artifacts_before_interrupt(&recorded.events);
    assert_eq!(requests.load(Ordering::SeqCst), 2);
}

/// A sequential declaration overrides the parallel default.
#[test]
fn maestro_sequential_tool_overrides_parallel_default() {
    scheduling(ToolExecutionMode::Sequential, false);
}
/// A mixed batch is sequential when one member requires it.
#[test]
fn maestro_mixed_tool_batch_runs_sequentially() {
    scheduling(ToolExecutionMode::Sequential, true);
}
/// Parallel declarations allow overlap through named gates.
#[test]
fn maestro_parallel_tools_overlap() {
    scheduling(ToolExecutionMode::Parallel, false);
}

/// Poll the producer while the first execution is held before checking admission.
fn scheduling(mode: ToolExecutionMode, mixed: bool) {
    let parallel = mode == ToolExecutionMode::Parallel;
    let (release, gate) = tokio::sync::oneshot::channel();
    let gate = Arc::new(Mutex::new(Some(gate)));
    let started = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&started);
    let echo = tool(
        "slow",
        Arc::new(move |id, _, _, _| {
            count.fetch_add(1, Ordering::SeqCst);
            let wait = (id == "tool-1").then(|| gate.lock().unwrap().take().unwrap());
            Box::pin(async move {
                if let Some(wait) = wait {
                    wait.await.unwrap();
                }
                Ok(artifact(&id, true))
            })
        }),
    );
    echo.write().unwrap().execution_mode = Some(if parallel {
        ToolExecutionMode::Parallel
    } else {
        ToolExecutionMode::Sequential
    });
    let count = Arc::clone(&started);
    let fast = tool(
        "fast",
        Arc::new(move |id, _, _, _| {
            count.fetch_add(1, Ordering::SeqCst);
            ready(Ok(artifact(&id, true)))
        }),
    );
    let names = if mixed {
        vec!["slow", "fast"]
    } else {
        vec!["slow", "slow"]
    };
    let mut config = config();
    config.tool_execution = None;
    let tasks = Arc::new(owned::Tasks::default());
    let stream = maestro_agent::agent_loop(
        vec![],
        tools(vec![echo, fast]),
        Arc::new(config),
        responses(vec![calls(&names, &serde_json::json!({}))]),
        tasks.spawner(),
    );
    let mut task = tasks.take();
    assert!(poll(task.as_mut()).is_pending());
    assert_eq!(started.load(Ordering::SeqCst), if parallel { 2 } else { 1 });
    release.send(()).unwrap();
    run(task);
    let recorded = drain(&stream);
    assert_eq!(started.load(Ordering::SeqCst), 2);
    assert_eq!(
        recorded.messages[1..].iter().map(text).collect::<Vec<_>>(),
        ["tool-1", "tool-2"]
    );
}

/// A true stop hook prevents subsequent steering and follow-up polls.
#[test]
fn maestro_stop_hook_prevents_later_queue_poll() {
    let steering = Arc::new(AtomicUsize::new(0));
    let follow = Arc::new(AtomicUsize::new(0));
    let mut config = config();
    let count = Arc::clone(&steering);
    config.get_steering_messages = Some(Arc::new(move || {
        count.fetch_add(1, Ordering::SeqCst);
        ready(Ok(vec![]))
    }));
    let count = Arc::clone(&follow);
    config.get_follow_up_messages = Some(Arc::new(move || {
        count.fetch_add(1, Ordering::SeqCst);
        ready(Ok(vec![user("follow")]))
    }));
    config.should_stop_after_turn = Some(Arc::new(|call| {
        assert_eq!(call.tool_results.len(), 1);
        assert_eq!(
            roles(call.new_messages),
            ["user", "assistant", "toolResult"]
        );
        assert_eq!(
            roles(&call.context.messages.read().unwrap()),
            ["user", "assistant", "toolResult"]
        );
        ready(Ok(true))
    }));
    let echo = tool(
        "echo",
        Arc::new(|_, _, _, _| ready(Ok(artifact("hello", false)))),
    );
    let context = tools(vec![echo]);
    context.messages.write().unwrap().clear();
    let recorded = observe(
        vec![user("Hello")],
        context,
        config,
        responses(vec![calls(&["echo"], &serde_json::json!({}))]),
    );
    assert_eq!(steering.load(Ordering::SeqCst), 1);
    assert_eq!(follow.load(Ordering::SeqCst), 0);
    assert_eq!(
        recorded.events.iter().map(kind).collect::<Vec<_>>(),
        [
            "agent_start",
            "turn_start",
            "message_start",
            "message_end",
            "message_start",
            "message_end",
            "tool_execution_start",
            "tool_execution_end",
            "message_start",
            "message_end",
            "turn_end",
            "agent_end"
        ]
    );
}

/// Unanimous termination suppresses automatic follow-up.
#[test]
fn maestro_all_terminated_tools_stop_follow_up() {
    termination(Termination::All, false);
}
/// A nonterminating member keeps automatic continuation.
#[test]
fn maestro_mixed_termination_continues() {
    termination(Termination::Mixed, false);
}
/// The after-hook's effective result controls termination.
#[test]
fn maestro_after_hook_can_terminate_batch() {
    termination(Termination::All, true);
}
/// Observe request cardinality through controlled producer completion.
fn termination(termination: Termination, after: bool) {
    let all = matches!(termination, Termination::All);
    let echo = tool(
        "echo",
        Arc::new(move |id, _, _, _| ready(Ok(artifact(&id, !after && (all || id == "tool-1"))))),
    );
    let mut config = config();
    config.tool_execution = None;
    if after {
        config.after_tool_call = Some(Arc::new(|_, _| {
            ready(Ok(Some(maestro_agent::AfterToolCallResult {
                terminate: Some(true),
                ..Default::default()
            })))
        }));
    }
    let first = calls(
        if all {
            &["echo"][..]
        } else {
            &["echo", "echo"][..]
        },
        &serde_json::json!({}),
    );
    let requests = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&requests);
    let options = maestro_agent::AgentLoopOptions {
        signal: None,
        stream_fn: Some(Arc::new(move |_, _, _, _| {
            let message = if count.fetch_add(1, Ordering::SeqCst) == 0 {
                Arc::clone(&first)
            } else {
                assistant("final")
            };
            ready(Ok(model_stream(message)))
        })),
    };
    let recorded = observe(vec![user("Hello")], tools(vec![echo]), config, options);
    assert_eq!(requests.load(Ordering::SeqCst), if all { 1 } else { 2 });
    assert_eq!(
        recorded
            .events
            .iter()
            .filter(|e| matches!(e, AgentEvent::TurnEnd { .. }))
            .count(),
        if all { 1 } else { 2 }
    );
    assert_eq!(recorded.messages.len(), if all { 3 } else { 5 });
    assert_eq!(
        roles(&recorded.messages),
        if all {
            vec!["user", "assistant", "toolResult"]
        } else {
            vec!["user", "assistant", "toolResult", "toolResult", "assistant"]
        }
    );
    if !all {
        assert_eq!(text(recorded.messages.last().unwrap()), "final");
    }
}

/// Empty continuation is rejected synchronously without submission.
#[test]
fn maestro_raw_continue_rejects_empty_context() {
    let context = context();
    context.messages.write().unwrap().clear();
    let tasks = Arc::new(owned::Tasks::default());
    let error = agent_loop_continue(
        context,
        Arc::new(config()),
        responses(vec![]),
        tasks.spawner(),
    )
    .err()
    .unwrap();
    assert_eq!(error.message, "Cannot continue: no messages in context");
    assert!(tasks.0.lock().unwrap().is_empty());
}
/// Existing user events are not replayed, and the supplied history appends.
#[test]
fn maestro_continue_does_not_replay_user_events() {
    let context = context();
    let history = Arc::clone(&context.messages);
    let tasks = Arc::new(owned::Tasks::default());
    let stream = agent_loop_continue(
        context,
        Arc::new(config()),
        responses(vec![assistant("continued")]),
        tasks.spawner(),
    )
    .unwrap();
    run(tasks.take());
    let recorded = drain(&stream);
    assert_eq!(
        recorded.messages.iter().map(text).collect::<Vec<_>>(),
        ["continued"]
    );
    assert_eq!(
        recorded.events.iter().map(kind).collect::<Vec<_>>(),
        [
            "agent_start",
            "turn_start",
            "message_start",
            "message_end",
            "turn_end",
            "agent_end"
        ]
    );
    let AgentMessage::Assistant(stored) = &history.read().unwrap()[1] else {
        panic!("missing stored assistant")
    };
    let AgentMessage::Assistant(result) = &recorded.messages[0] else {
        panic!("missing returned assistant")
    };
    assert!(Arc::ptr_eq(stored, result));
}
/// Custom tails are admitted before their first conversion.
#[test]
fn maestro_continue_accepts_convertible_custom_tail() {
    let (context, mut config) = notice_context();
    let converted = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&converted);
    config.convert_to_llm = Arc::new(move |entries| {
        count.fetch_add(1, Ordering::SeqCst);
        assert!(matches!(
            entries.read().unwrap()[0],
            AgentMessage::Custom(_)
        ));
        ready(Ok(vec![match user("converted") {
            AgentMessage::User(value) => Message::User(value.read().unwrap().clone()),
            _ => unreachable!(),
        }]))
    });
    let tasks = Arc::new(owned::Tasks::default());
    let options = maestro_agent::AgentLoopOptions {
        signal: None,
        stream_fn: Some(Arc::new(|_, request, _, _| {
            let Message::User(value) = &request.messages[0] else {
                panic!("conversion missing")
            };
            assert_eq!(value.content, UserContent::Text("converted".into()));
            ready(Ok(model_stream(assistant("done"))))
        })),
    };
    let stream = agent_loop_continue(context, Arc::new(config), options, tasks.spawner()).unwrap();
    assert_eq!(converted.load(Ordering::SeqCst), 0);
    run(tasks.take());
    assert_eq!(converted.load(Ordering::SeqCst), 1);
    assert_eq!(drain(&stream).messages.len(), 1);
}
/// A caller-defined notification role.
struct Notice;
impl CustomAgentMessages for Notice {
    fn role(&self) -> &'static str {
        "notification"
    }
}
/// Construct a custom-tail context and its complete caller configuration.
fn notice_context() -> (AgentContext<Notice>, AgentLoopConfig<Notice>) {
    let ordinary = config();
    (
        AgentContext {
            system_prompt: "instructions".into(),
            messages: Arc::new(RwLock::new(vec![AgentMessage::Custom(Arc::new(
                RwLock::new(Notice),
            ))])),
            tools: None,
        },
        AgentLoopConfig {
            model: ordinary.model,
            options: ordinary.options,
            extra: ordinary.extra,
            objects: ordinary.objects,
            thinking_budgets: None,
            convert_to_llm: Arc::new(|_| ready(Ok(vec![]))),
            transform_context: None,
            get_api_key: None,
            before_tool_call: None,
            after_tool_call: None,
            should_stop_after_turn: None,
            prepare_next_turn: None,
            get_steering_messages: None,
            get_follow_up_messages: None,
            tool_execution: None,
        },
    )
}
/// Publish a controlled authoritative response using the existing model stream.
fn model_stream(
    message: maestro_models::SharedAssistantMessage,
) -> maestro_models::AssistantMessageEventStream {
    let stream = maestro_models::AssistantMessageEventStream::new();
    stream.end(Some(message));
    stream
}

/// Effective batch termination outcome.
#[derive(Clone, Copy)]
enum Termination {
    /// Every result terminates.
    All,
    /// Only the first result terminates.
    Mixed,
}

/// Turn results retain source order despite reversed execution completion.
fn assert_source_results(events: &[AgentEvent]) {
    let results = events
        .iter()
        .find_map(|e| match e {
            AgentEvent::TurnEnd { tool_results, .. } => Some(tool_results),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        results
            .iter()
            .map(|r| r.tool_call_id.as_str())
            .collect::<Vec<_>>(),
        ["tool-1", "tool-2"]
    );
    let artifacts = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::MessageEnd {
                message: AgentMessage::ToolResult(value),
            } => Some(value.read().unwrap().tool_call_id.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(artifacts, ["tool-1", "tool-2"]);
}

/// Both artifact message pairs precede the queued interruption.
fn assert_artifacts_before_interrupt(events: &[AgentEvent]) {
    let interrupt = events.iter().position(|e| matches!(e,AgentEvent::MessageStart { message } if matches!(message,AgentMessage::User(_)))).unwrap();
    let artifacts = events
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            matches!(
                e,
                AgentEvent::MessageStart {
                    message: AgentMessage::ToolResult(_)
                } | AgentEvent::MessageEnd {
                    message: AgentMessage::ToolResult(_)
                }
            )
            .then_some(i)
        })
        .collect::<Vec<_>>();
    assert_eq!(artifacts.len(), 4);
    assert!(artifacts.iter().all(|i| *i < interrupt));
}
