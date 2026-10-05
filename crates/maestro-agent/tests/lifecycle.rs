mod support;
use maestro_agent::*;
use maestro_models::*;
use support::*;

#[tokio::test]
async fn prompt_emits_ordered_events_and_terminal_history() {
    let config = AgentOptions {
        context: AgentContext {
            system_prompt: Some("instructions".into()),
            messages: vec![],
        },
        ..Default::default()
    };
    let (agent, provider) = setup(vec![Script::Steps(steps("hello"))], config);
    let events = capture(&agent);
    let mut request = options();
    request.session_affinity = Some("session".into());
    request.temperature = Some(0.3);
    request.output_limit = Some(0);
    let input = user("question");
    let result = agent.prompt(input.clone(), request).unwrap().await.unwrap();
    assert_eq!(
        events.lock().unwrap().iter().map(label).collect::<Vec<_>>(),
        vec![
            "agent_start",
            "turn_start",
            "message_start",
            "message_end",
            "message_start",
            "message_update",
            "message_update",
            "message_update",
            "message_end",
            "turn_end",
            "agent_end"
        ]
    );
    assert_eq!(result[0], input);
    let AgentMessage::Model(Message::Assistant(terminal)) = &result[1] else {
        panic!()
    };
    assert_eq!(terminal.timestamp, 42);
    assert_eq!(terminal.stop_reason, Some(StopReason::Stop));
    assert_eq!(agent.state().context.messages, result);
    assert!(!agent.state().is_running);
    assert!(agent.state().streaming_message.is_none());
    let events = events.lock().unwrap();
    assert_eq!(
        events[8],
        AgentEvent::MessageEnd {
            message: result[1].clone()
        }
    );
    assert_eq!(
        events[9],
        AgentEvent::TurnEnd {
            message: terminal.clone(),
            tool_results: vec![]
        }
    );
    assert_eq!(events[10], AgentEvent::AgentEnd { messages: result });
    let calls = provider.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].context.system_prompt.as_deref(),
        Some("instructions")
    );
    assert!(calls[0].context.tools.is_empty());
    assert_eq!(
        calls[0].options.session_affinity.as_deref(),
        Some("session")
    );
    assert_eq!(calls[0].options.temperature, Some(0.3));
    assert_eq!(calls[0].options.output_limit, Some(0));
}

#[tokio::test]
async fn continue_uses_history_without_reemitting_it() {
    for history in [
        user("existing"),
        AgentMessage::Model(Message::ToolResult(ToolResultMessage {
            tool_call_id: "call".into(),
            tool_name: "lookup".into(),
            content: vec![],
            details: None,
            is_error: false,
            timestamp: 19,
        })),
    ] {
        let config = AgentOptions {
            context: AgentContext {
                messages: vec![history.clone()],
                ..Default::default()
            },
            ..Default::default()
        };
        let (agent, provider) = setup(vec![Script::Steps(steps("continued"))], config);
        let events = capture(&agent);
        let result = agent.continue_run(options()).unwrap().await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(
            agent.state().context.messages,
            vec![history.clone(), result[0].clone()]
        );
        let AgentMessage::Model(record) = history else {
            panic!()
        };
        assert_eq!(provider.calls()[0].context.messages, vec![record]);
        let events = events.lock().unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, AgentEvent::MessageStart { .. }))
                .count(),
            1
        );
        assert_eq!(
            events.last(),
            Some(&AgentEvent::AgentEnd { messages: result })
        );
    }
}

#[tokio::test]
async fn overlapping_prompt_and_continue_are_busy() {
    let (wait, entered, release) = gate();
    let mut response = vec![wait];
    response.extend(steps("done"));
    let (agent, provider) = setup(vec![Script::Steps(response)], AgentOptions::default());
    let events = capture(&agent);
    let handle = agent.prompt(user("first"), options()).unwrap();
    entered.await.unwrap();
    agent.steer(user("queued steering"));
    agent.follow_up(user("queued follow-up"));
    let state = agent.state();
    let count = events.lock().unwrap().len();
    assert!(matches!(
        agent.prompt(user("second"), options()),
        Err(AgentError::Busy)
    ));
    assert!(matches!(
        agent.continue_run(options()),
        Err(AgentError::Busy)
    ));
    assert_eq!(agent.state(), state);
    assert_eq!(events.lock().unwrap().len(), count);
    assert_eq!(provider.calls().len(), 1);
    assert_eq!(agent.queue(Queue::Steering), vec![user("queued steering")]);
    assert_eq!(agent.queue(Queue::FollowUp), vec![user("queued follow-up")]);
    agent.clear_queue(Queue::Steering);
    agent.clear_queue(Queue::FollowUp);
    release.send(()).unwrap();
    handle.await.unwrap();
}

#[tokio::test]
async fn continue_requires_history_and_queued_input_for_assistant_tail() {
    let (empty, provider) = setup(vec![], AgentOptions::default());
    empty.steer(user("queued"));
    assert!(matches!(
        empty.continue_run(options()),
        Err(AgentError::NoUsableHistory)
    ));
    assert_eq!(empty.queue(Queue::Steering), vec![user("queued")]);
    assert!(provider.calls().is_empty());
    let (agent, provider) = setup(vec![Script::Steps(steps("tail"))], AgentOptions::default());
    agent
        .prompt(user("first"), options())
        .unwrap()
        .await
        .unwrap();
    let history = agent.state();
    assert!(matches!(
        agent.continue_run(options()),
        Err(AgentError::AssistantTail)
    ));
    assert_eq!(agent.state(), history);
    assert_eq!(provider.calls().len(), 1);
}

#[tokio::test]
async fn scripted_model_adapters_swap_without_caller_changes() {
    async fn caller(script: Script) -> Vec<AgentMessage> {
        let (agent, _) = setup(vec![script], AgentOptions::default());
        agent
            .prompt(user("same caller"), options())
            .unwrap()
            .await
            .unwrap()
    }
    let explicit = caller(Script::Steps(steps("same response"))).await;
    let factory = caller(Script::Factory(Box::new(|call| {
        Box::pin(async move {
            assert_eq!(call.context.messages.len(), 1);
            Ok(steps("same response"))
        })
    })))
    .await;
    assert_eq!(explicit, factory);
}

#[tokio::test]
async fn successive_prompts_preserve_conversation_context() {
    let (agent, provider) = setup(
        vec![
            Script::Steps(steps("first answer")),
            Script::Factory(Box::new(|call| {
                Box::pin(async move {
                    assert_eq!(call.context.messages.len(), 3);
                    assert_eq!(
                        call.context.messages[0],
                        match user("first") {
                            AgentMessage::Model(m) => m,
                            _ => panic!(),
                        }
                    );
                    assert!(matches!(call.context.messages[1], Message::Assistant(_)));
                    assert_eq!(
                        call.context.messages[2],
                        match user("second") {
                            AgentMessage::Model(m) => m,
                            _ => panic!(),
                        }
                    );
                    Ok(steps("second answer"))
                })
            })),
        ],
        AgentOptions::default(),
    );
    let first = agent
        .prompt(user("first"), options())
        .unwrap()
        .await
        .unwrap();
    let events = capture(&agent);
    let second = agent
        .prompt(user("second"), options())
        .unwrap()
        .await
        .unwrap();
    assert_eq!(second.len(), 2);
    assert_eq!(
        agent.state().context.messages,
        [first, second.clone()].concat()
    );
    assert_eq!(
        events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| matches!(e, AgentEvent::MessageEnd { .. }))
            .count(),
        2
    );
    assert_eq!(provider.calls().len(), 2);
}
