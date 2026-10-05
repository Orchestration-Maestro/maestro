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
        input: vec!["text".into()],
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
