mod support;
use maestro_agent::*;
use maestro_models::*;
use support::*;

#[tokio::test]
async fn updates_preserve_cumulative_outer_and_nested_snapshots() {
    let response = vec![
        ProviderUpdate::ThinkingStart {
            content_index: 0,
            signature: Some("signature".into()),
        },
        ProviderUpdate::ThinkingDelta {
            content_index: 0,
            delta: "思".into(),
        },
        ProviderUpdate::ThinkingEnd { content_index: 0 },
        ProviderUpdate::TextStart { content_index: 1 },
        ProviderUpdate::TextDelta {
            content_index: 1,
            delta: "é".into(),
        },
        ProviderUpdate::TextDelta {
            content_index: 1,
            delta: "🙂".into(),
        },
        ProviderUpdate::TextEnd {
            content_index: 1,
            replay_metadata: None,
        },
        ProviderUpdate::Usage {
            usage: Usage {
                input: 11,
                output: 7,
                ..Default::default()
            },
        },
        ProviderUpdate::ResponseIdentity {
            response_model: Some("actual".into()),
            response_id: Some("response".into()),
        },
        ProviderUpdate::Done {
            reason: StopReason::Length,
        },
    ];
    let (agent, _) = setup(
        vec![Script::Steps(
            response.into_iter().map(ScriptStep::Update).collect(),
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
                assistant_message_event: ModelEvent::TextDelta { partial, .. },
            } => {
                assert_eq!(message, partial);
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
            replay_metadata: None
        })
    );
    assert_eq!(
        deltas[1].content[1],
        AssistantContent::Text(TextContent {
            text: "é🙂".into(),
            replay_metadata: None
        })
    );
    assert_eq!(
        deltas[0].content[0],
        AssistantContent::Thinking(ThinkingContent::Readable {
            text: "思".into(),
            signature: Some("signature".into())
        })
    );
    let AgentMessage::Model(Message::Assistant(terminal)) = &result[1] else {
        panic!()
    };
    assert_eq!(terminal.content, deltas[1].content);
    assert_eq!(terminal.usage.input, 11);
    assert_eq!(terminal.usage.output, 7);
    assert_eq!(terminal.response_id.as_deref(), Some("response"));
    assert_eq!(terminal.response_model.as_deref(), Some("actual"));
    assert_eq!(terminal.stop_reason, Some(StopReason::Length));
    assert_eq!(deltas[1].usage, Usage::default());
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
    let (agent, provider) = setup(vec![Script::Steps(steps("response"))], config);
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
    let (agent, provider) = setup(vec![Script::Steps(steps("reply"))], config);
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
