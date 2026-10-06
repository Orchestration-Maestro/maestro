mod support;
use maestro_agent::*;
use maestro_models::*;
use support::*;

#[tokio::test]
async fn updates_preserve_cumulative_outer_and_nested_snapshots() {
    let response = vec![
        Update::ThinkingStart {
            content_index: 0,
            signature: Some("signature".into()),
        },
        Update::ThinkingDelta {
            content_index: 0,
            delta: "思".into(),
        },
        Update::ThinkingEnd { content_index: 0 },
        Update::TextStart { content_index: 1 },
        Update::TextDelta {
            content_index: 1,
            delta: "é".into(),
        },
        Update::TextDelta {
            content_index: 1,
            delta: "🙂".into(),
        },
        Update::TextEnd {
            content_index: 1,
            text_signature: None,
        },
        Update::Usage {
            usage: Usage {
                input: 11.0,
                output: 7.0,
                ..zero_usage()
            },
        },
        Update::ResponseIdentity {
            response_model: Some("actual".into()),
            response_id: Some("response".into()),
        },
        Update::Done {
            reason: StopReason::Length,
        },
    ];
    let (agent, _) = setup(
        vec![Response::Steps(
            response.into_iter().map(Action::Update).collect(),
        )],
        AgentOptions::default(),
    );
    let events = capture(&agent);
    let result = agent
        .prompt(user("unicode"), options())
        .unwrap()
        .await
        .unwrap();
    let events = events.lock().unwrap();
    let deltas: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::MessageUpdate {
                message,
                assistant_message_event: AssistantMessageEvent::TextDelta { partial, .. },
            } => {
                assert_eq!(message, &*partial.read().unwrap());
                Some(message.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(deltas.len(), 2);
    assert_eq!(
        deltas[0].content[1],
        AssistantContent::Text(TextContent {
            text: "é".into(),
            text_signature: None
        })
    );
    assert_eq!(
        deltas[1].content[1],
        AssistantContent::Text(TextContent {
            text: "é🙂".into(),
            text_signature: None
        })
    );
    assert_eq!(
        deltas[0].content[0],
        AssistantContent::Thinking(ThinkingContent {
            thinking: "思".into(),
            thinking_signature: Some("signature".into()),
            redacted: None
        })
    );
    let AgentMessage::Model(Message::Assistant(terminal)) = &result[1] else {
        panic!()
    };
    assert_eq!(terminal.content, deltas[1].content);
    assert_eq!(terminal.usage.input, 11.0);
    assert_eq!(terminal.usage.output, 7.0);
    assert_eq!(terminal.response_id.as_deref(), Some("response"));
    assert_eq!(terminal.response_model.as_deref(), Some("actual"));
    assert_eq!(terminal.stop_reason, StopReason::Length);
    assert_eq!(deltas[1].usage, zero_usage());
    let mut snapshot = agent.state();
    snapshot.context.messages.clear();
    assert_eq!(agent.state().context.messages, result);
}

#[tokio::test]
async fn context_transform_precedes_application_message_conversion() {
    use std::sync::{Arc, Mutex};
    let raw = AgentMessage::Application {
        kind: "custom".into(),
        data: serde_json::json!({"text":"raw"}),
        timestamp: 9,
    };
    let order = Arc::new(Mutex::new(vec![]));
    let first = order.clone();
    let expected = raw.clone();
    let second = order.clone();
    let config = AgentOptions {
        context: AgentContext {
            messages: vec![raw.clone()],
            ..Default::default()
        },
        transform_context: Some(Arc::new(move |records, signal| {
            assert!(!signal.is_cancelled());
            assert_eq!(records, vec![expected.clone()]);
            first.lock().unwrap().push("transform");
            Box::pin(async {
                vec![
                    user("transformed"),
                    AgentMessage::Application {
                        kind: "filter".into(),
                        data: serde_json::Value::Null,
                        timestamp: 10,
                    },
                ]
            })
        })),
        convert_messages: Some(Arc::new(move |records| {
            assert_eq!(records[0], user("transformed"));
            assert_eq!(records.len(), 2);
            second.lock().unwrap().push("convert");
            Box::pin(async move {
                vec![match user("converted") {
                    AgentMessage::Model(m) => m,
                    _ => panic!(),
                }]
            })
        })),
        ..Default::default()
    };
    let (agent, provider) = setup(vec![Response::Steps(steps("response"))], config);
    let result = agent.continue_run(options()).unwrap().await.unwrap();
    assert_eq!(*order.lock().unwrap(), vec!["transform", "convert"]);
    assert_eq!(
        provider.calls()[0].context.messages,
        vec![match user("converted") {
            AgentMessage::Model(m) => m,
            _ => panic!(),
        }]
    );
    assert_eq!(agent.state().context.messages, vec![raw, result[0].clone()]);
}

#[tokio::test]
async fn default_conversion_filters_application_records() {
    let custom = AgentMessage::Application {
        kind: "arbitrary/new-kind".into(),
        data: serde_json::json!([1, {"opaque":true}]),
        timestamp: 12,
    };
    let config = AgentOptions {
        context: AgentContext {
            messages: vec![user("old"), custom.clone()],
            ..Default::default()
        },
        ..Default::default()
    };
    let (agent, provider) = setup(vec![Response::Steps(steps("reply"))], config);
    let result = agent.prompt(user("new"), options()).unwrap().await.unwrap();
    assert_eq!(
        provider.calls()[0].context.messages,
        [user("old"), user("new")]
            .into_iter()
            .map(|m| match m {
                AgentMessage::Model(m) => m,
                _ => panic!(),
            })
            .collect::<Vec<_>>()
    );
    assert_eq!(
        agent.state().context.messages,
        [vec![user("old"), custom], result].concat()
    );
}

#[tokio::test]
async fn tool_call_mutations_do_not_escape_agent_snapshot_seams() {
    use std::sync::Arc;
    let agent = tool_call_agent(AgentOptions::default());
    let observer = agent.clone();
    agent.subscribe(Arc::new(move |event, _| {
        if let Some(message) = observer.state().streaming_message {
            change_call(&message, 99);
        }
        match event {
            AgentEvent::MessageUpdate {
                message,
                assistant_message_event:
                    AssistantMessageEvent::ToolcallEnd {
                        tool_call, partial, ..
                    },
            } => {
                change_call(&message, 88);
                change_call(&partial.read().unwrap(), 77);
                tool_call.write().unwrap().arguments["x"] = serde_json::json!(66);
            }
            AgentEvent::MessageEnd {
                message: AgentMessage::Model(Message::Assistant(message)),
            } => change_call(&message, 55),
            _ => {}
        }
        Box::pin(async {})
    }));
    let events = capture(&agent);
    let result = agent
        .prompt(user("input"), options())
        .unwrap()
        .await
        .unwrap();
    let AgentMessage::Model(Message::Assistant(terminal)) = &result[1] else {
        panic!()
    };
    assert_call(terminal, 1);
    for event in events.lock().unwrap().iter() {
        if let AgentEvent::MessageUpdate {
            message,
            assistant_message_event:
                AssistantMessageEvent::ToolcallEnd {
                    tool_call, partial, ..
                },
        } = event
        {
            assert_call(message, 1);
            assert_call(&partial.read().unwrap(), 1);
            assert_eq!(tool_call.read().unwrap().arguments["x"], 1);
        }
    }
    let state = agent.state();
    let AgentMessage::Model(Message::Assistant(message)) = &state.context.messages[1] else {
        panic!()
    };
    change_call(message, 44);
    let fresh = agent.state();
    let AgentMessage::Model(Message::Assistant(message)) = &fresh.context.messages[1] else {
        panic!()
    };
    assert_call(message, 1);
}
fn change_call(message: &AssistantMessage, value: i32) {
    let AssistantContent::ToolCall(call) = &message.content[0] else {
        panic!()
    };
    call.write().unwrap().arguments["x"] = serde_json::json!(value);
}
fn assert_call(message: &AssistantMessage, value: i32) {
    let AssistantContent::ToolCall(call) = &message.content[0] else {
        panic!()
    };
    assert_eq!(call.read().unwrap().arguments["x"], value);
}

#[tokio::test]
async fn stop_hook_mutation_preserves_result_history_and_agent_end() {
    use std::sync::Arc;
    let agent = tool_call_agent(AgentOptions {
        stop_after_turn: Some(Arc::new(|context| {
            change_call(&context.message, 222);
            Box::pin(async { true })
        })),
        ..Default::default()
    });
    let events = capture(&agent);
    let result = agent
        .prompt(user("input"), options())
        .unwrap()
        .await
        .unwrap();
    let AgentMessage::Model(Message::Assistant(terminal)) = &result[1] else {
        panic!()
    };
    assert_call(terminal, 1);
    assert_eq!(agent.state().context.messages, result);
    assert!(events.lock().unwrap().iter().any(|event| matches!(event,
        AgentEvent::AgentEnd { messages } if messages == &result
    )));
}

fn tool_call_agent(config: AgentOptions) -> Agent {
    use std::sync::{Arc, RwLock};
    let model: Model = serde_json::from_value(serde_json::json!({
        "id":"controlled","name":"Controlled","api":"controlled","provider":"local",
        "baseUrl":"","reasoning":false,"input":["text"],
        "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},
        "contextWindow":0,"maxTokens":0
    }))
    .unwrap();
    let supplied: ApiStreamSimpleFunction = Arc::new(|model, _, _| {
        let call = Arc::new(RwLock::new(ToolCall {
            id: "call".into(),
            name: "lookup".into(),
            arguments: serde_json::json!({"x":1}).as_object().unwrap().clone(),
            thought_signature: None,
        }));
        let message = Arc::new(RwLock::new(AssistantMessage {
            content: vec![AssistantContent::ToolCall(call.clone())],
            api: model.api,
            provider: model.provider,
            model: model.id,
            response_model: None,
            response_id: None,
            diagnostics: None,
            usage: zero_usage(),
            stop_reason: StopReason::Stop,
            error_message: None,
            timestamp: 42.0,
        }));
        let stream = create_assistant_message_event_stream();
        stream
            .push(AssistantMessageEvent::Start {
                partial: message.clone(),
            })
            .unwrap();
        stream
            .push(AssistantMessageEvent::ToolcallEnd {
                content_index: 0.0,
                tool_call: call,
                partial: message.clone(),
            })
            .unwrap();
        stream
            .push(AssistantMessageEvent::Done {
                reason: StopReason::Stop,
                message,
            })
            .unwrap();
        Ok(stream)
    });
    Agent::new(supplied, model, config)
}

#[test]
fn admitted_tool_calls_do_not_alias_caller_history_or_queues() {
    let message: AssistantMessage = serde_json::from_value(serde_json::json!({
        "role":"assistant","content":[{"type":"toolCall","id":"call","name":"lookup","arguments":{"x":1}}],
        "api":"controlled","provider":"local","model":"controlled",
        "usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,
                 "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},
        "stopReason":"stop","timestamp":0
    })).unwrap();
    let original = AgentMessage::Model(Message::Assistant(message.clone()));
    let (agent, _) = setup(
        vec![],
        AgentOptions {
            context: AgentContext {
                messages: vec![original.clone()],
                ..Default::default()
            },
            ..Default::default()
        },
    );
    agent.steer(original.clone());
    agent.follow_up(original);
    change_call(&message, 999);
    let state = agent.state();
    let AgentMessage::Model(Message::Assistant(admitted)) = &state.context.messages[0] else {
        panic!()
    };
    assert_call(admitted, 1);
    for queue in [Queue::Steering, Queue::FollowUp] {
        let records = agent.queue(queue);
        assert_eq!(records.len(), 1);
        let AgentMessage::Model(Message::Assistant(admitted)) = &records[0] else {
            panic!()
        };
        assert_call(admitted, 1);
    }
}
