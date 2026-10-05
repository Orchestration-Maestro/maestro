#![allow(dead_code)]
use maestro_agent::*;
use maestro_models::*;
use std::sync::{Arc, Mutex};

pub fn user(text: &str) -> AgentMessage {
    AgentMessage::Model(Message::User(UserMessage {
        content: vec![InputContent::Text(TextContent {
            text: text.into(),
            replay_metadata: None,
        })],
        timestamp: 17,
    }))
}
pub fn options() -> StreamOptions {
    StreamOptions {
        auth: Some(RequestAuth::ConfiguredWithoutSecret { source: None }),
        ..Default::default()
    }
}
pub fn steps(text: &str) -> Vec<ScriptStep> {
    vec![
        ScriptStep::Update(ProviderUpdate::TextStart { content_index: 0 }),
        ScriptStep::Update(ProviderUpdate::TextDelta {
            content_index: 0,
            delta: text.into(),
        }),
        ScriptStep::Update(ProviderUpdate::TextEnd {
            content_index: 0,
            replay_metadata: None,
        }),
        ScriptStep::Update(ProviderUpdate::Done {
            reason: StopReason::Stop,
        }),
    ]
}
pub fn setup(scripts: Vec<Script>, config: AgentOptions) -> (Agent, Arc<ScriptedProvider>) {
    let model = Model {
        identity: ModelIdentity {
            provider: "script".into(),
            model: "text".into(),
            operation: "chat".into(),
        },
        name: "Synthetic text".into(),
        endpoint: "synthetic:endpoint".into(),
        chat: Some(ChatMetadata {
            context_window: None,
        }),
        protocol: "synthetic".into(),
        rates: None,
        headers: Default::default(),
        input: vec!["text".into(), "image".into()],
        capabilities: RequestCapabilities {
            session_affinity: true,
            temperature: true,
            ..Default::default()
        },
    };
    let provider = Arc::new(ScriptedProvider::new(scripts));
    let mut models = Models::new(Arc::new(|| 42));
    models.register(model.clone(), provider.clone()).unwrap();
    (Agent::new(Arc::new(models), model, config), provider)
}
pub fn capture(agent: &Agent) -> Arc<Mutex<Vec<AgentEvent>>> {
    let events = Arc::new(Mutex::new(Vec::new()));
    let output = events.clone();
    agent.subscribe(Arc::new(move |event, _| {
        output.lock().unwrap().push(event);
        Box::pin(async {})
    }));
    events
}
pub fn label(event: &AgentEvent) -> &'static str {
    match event {
        AgentEvent::ToolExecutionStart { .. } => "tool_start",
        AgentEvent::ToolExecutionUpdate { .. } => "tool_update",
        AgentEvent::ToolExecutionEnd { .. } => "tool_end",
        AgentEvent::AgentStart => "agent_start",
        AgentEvent::AgentEnd { .. } => "agent_end",
        AgentEvent::TurnStart => "turn_start",
        AgentEvent::TurnEnd { .. } => "turn_end",
        AgentEvent::MessageStart { .. } => "message_start",
        AgentEvent::MessageEnd { .. } => "message_end",
        AgentEvent::MessageUpdate { .. } => "message_update",
    }
}

pub fn gate() -> (
    ScriptStep,
    tokio::sync::oneshot::Receiver<()>,
    tokio::sync::oneshot::Sender<()>,
) {
    let (entered_tx, entered) = tokio::sync::oneshot::channel();
    let (release, release_rx) = tokio::sync::oneshot::channel();
    (
        ScriptStep::Wait(Box::pin(async move {
            entered_tx.send(()).unwrap();
            release_rx.await.unwrap();
        })),
        entered,
        release,
    )
}
pub async fn pending<F: std::future::Future + Unpin>(future: &mut F) {
    std::future::poll_fn(|cx| {
        assert!(std::pin::Pin::new(&mut *future).poll(cx).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
}

pub fn call_steps(
    calls: &[(&str, &str, serde_json::Value)],
    reason: StopReason,
) -> Vec<ScriptStep> {
    let mut out = vec![];
    for (index, (id, name, args)) in calls.iter().enumerate() {
        out.extend([
            ScriptStep::Update(ProviderUpdate::ToolCallStart {
                content_index: index,
                id: (*id).into(),
                name: (*name).into(),
                replay_metadata: Some("replay".into()),
            }),
            ScriptStep::Update(ProviderUpdate::ToolCallDelta {
                content_index: index,
                delta: args.to_string(),
            }),
            ScriptStep::Update(ProviderUpdate::ToolCallEnd {
                content_index: index,
            }),
        ]);
    }
    out.push(ScriptStep::Update(ProviderUpdate::Done { reason }));
    out
}
pub fn output(text: &str) -> ToolResult {
    ToolResult {
        content: vec![InputContent::Text(TextContent {
            text: text.into(),
            replay_metadata: None,
        })],
        details: serde_json::json!({}),
        terminate: None,
    }
}
pub fn tool(name: &str, execute: ToolExecute) -> Tool {
    Tool {
        declaration: ToolDeclaration {
            name: name.into(),
            description: "synthetic".into(),
            parameters: serde_json::json!({"type":"object"}),
        },
        label: name.into(),
        prepare: None,
        execute,
        execution_mode: None,
    }
}
pub fn results(records: &[AgentMessage]) -> Vec<ToolResultMessage> {
    records
        .iter()
        .filter_map(|m| match m {
            AgentMessage::Model(Message::ToolResult(r)) => Some(r.clone()),
            _ => None,
        })
        .collect()
}
