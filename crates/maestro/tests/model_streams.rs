//! The native agent consumes a compiled provider's retained stream through the SDK route.
use maestro_app::{
    AgentEvent, AgentMessage, AgentSession, AssistantMessageEventStream, CreateAgentSessionOptions,
    ExtensionBindings, Local, NoTools, PromptOptions, ResourceLoaderOptions, SessionResources,
    SharedAssistantMessage,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    future::Future,
    rc::Rc,
    sync::Arc,
    task::{Context, Waker},
};
use tokio::sync::{mpsc, oneshot};

#[test]
fn registered_guest_streams_reach_the_native_agent() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    local.block_on(&runtime, async {
        let root = super::support::Root::new();
        let session = session(&root, local.clone()).await;
        command(&session, "agent", "registered").await;
        let (entered, mut incoming) = mpsc::unbounded_channel();
        let mut streams = observe_stream(&session, entered);
        let events = Rc::new(RefCell::new(Vec::new()));
        let finals = Rc::new(RefCell::new(Vec::new()));
        observe_agent(&session, &events, &finals);
        let prompt = session.prompt("native agent 雪", PromptOptions::default());
        let prompting = tokio::task::spawn_local(async move { prompt.await.unwrap() });
        let stream = streams.recv().await.unwrap();
        pending(&stream);
        let producing = session.clone();
        let producer =
            tokio::task::spawn_local(
                async move { command(&producing, "produce", "produced").await },
            );
        for expected in ["payload", "response"] {
            let (phase, resume) = incoming.recv().await.unwrap();
            assert_eq!(phase, expected);
            pending(&stream);
            resume.send(()).unwrap();
        }
        producer.await.unwrap();
        let result = stream.result().await;
        assert!(Arc::ptr_eq(&result, &stream.result().await));
        prompting.await.unwrap();
        check(&events.borrow(), &finals.borrow(), &result);
        assert!(stream.next().await.is_none());
        session.dispose();
    });
}

/// The application's host widgets over the default live theme.
fn widgets() -> Rc<dyn maestro_app::ExtensionWidgets> {
    let highlighter =
        Rc::new(maestro_theme::SyntectHighlighter::new().expect("bundled syntax definitions"));
    Rc::new(maestro_chat::ExtensionWidgetHost::new(
        maestro_theme::LiveTheme::default(),
        highlighter,
        maestro_tui::TerminalImage::default(),
    ))
}

/// Build an isolated SDK session with the production component factory and no ambient auth.
async fn session(
    root: &super::support::Root,
    local: Rc<tokio::task::LocalSet>,
) -> Rc<AgentSession> {
    let mut resources =
        ResourceLoaderOptions::<Local>::new(local, Rc::new(|_| Box::pin(async { Ok(None) })));
    resources
        .extension_factories
        .push(maestro_extensions_wasmtime::artifact_factory(
            std::env::var("MAESTRO_CONTEXT_COMPONENT").expect("nextest author-component setup"),
            widgets(),
            env!("CARGO_PKG_VERSION").into(),
        ));
    resources.system_prompt = Some(String::new());
    resources.append_system_prompt = Some(Vec::new());
    resources.offline = Some(true);
    let cwd = root.path("project").display().to_string();
    let created = maestro_app::create_agent_session(CreateAgentSessionOptions {
        cwd: Some(cwd.clone()), agent_dir: Some(root.path("agent").display().to_string()),
        auth_storage: Some(Arc::new(maestro_app::AuthStorage::in_memory(maestro_app::AuthStorageData::default()))),
        model_registry: None,
        model: Some(Arc::new(std::sync::RwLock::new(serde_json::from_value(json!({
            "id":"registered-case","name":"registered","api":"guest-owned-stream",
            "provider":"compiled-stream","baseUrl":"https://example.invalid","reasoning":false,"input":["text"],
            "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":10000,"maxTokens":10
        })).unwrap()))),
        thinking_level: None, scoped_models: Vec::new(), no_tools: Some(NoTools::All),
        tools: None, custom_tools: Vec::new(), resources: SessionResources::Default(Box::new(resources)),
        session_manager: Some(Rc::new(RefCell::new(maestro_app::SessionManager::in_memory(Some(&cwd)).unwrap()))),
        settings_manager: Some(Rc::new(RefCell::new(maestro_app::SettingsManager::in_memory(maestro_app::Settings::default())))),
        session_start_event: None, theme: maestro_theme::LiveTheme::default(), offline: Some(true),
    }).await.unwrap();
    assert!(created.extensions_result.errors.is_empty());
    created
        .session
        .bind_extensions(ExtensionBindings::default())
        .await
        .unwrap();
    created.session
}

/// Invoke the compiled command through the same runner owned by the application session.
async fn command(session: &AgentSession, action: &str, expected: &str) {
    let runner = session.extension_runner();
    let handler = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    assert_eq!(
        handler(
            format!("model-registered {action}"),
            runner.create_command_context()
        )
        .await
        .unwrap_err()
        .to_string(),
        expected
    );
}

/// Gate original native hooks while forwarding the SDK's actual registered-provider request.
fn observe_stream(
    session: &AgentSession,
    entered: mpsc::UnboundedSender<(&'static str, oneshot::Sender<()>)>,
) -> mpsc::UnboundedReceiver<AssistantMessageEventStream> {
    native_hooks(session, entered);
    let (streams, incoming) = mpsc::unbounded_channel();
    let original = session.agent().stream_fn.read().unwrap().clone().unwrap();
    *session.agent().stream_fn.write().unwrap() =
        Some(Rc::new(move |model, context, options, open| {
            assert_eq!(model.id, "registered-case");
            assert!(
                serde_json::to_value(&context.messages)
                    .unwrap()
                    .to_string()
                    .contains("native agent 雪")
            );
            assert!(!options.common.signal.as_ref().unwrap().is_aborted());
            let result = original(model, context, options, open);
            let streams = streams.clone();
            Box::pin(async move {
                let stream = result.await?;
                streams.send(stream.clone()).unwrap();
                Ok(stream)
            })
        }));
    incoming
}

/// Release only after the native controller observed the original callback's readiness.
async fn gate(
    entered: &mpsc::UnboundedSender<(&'static str, oneshot::Sender<()>)>,
    phase: &'static str,
) {
    let (resume, done) = oneshot::channel();
    entered.send((phase, resume)).unwrap();
    done.await.unwrap();
}

/// The independently observed result cannot settle while either hook is waiting.
fn pending(stream: &AssistantMessageEventStream) {
    let mut context = Context::from_waker(Waker::noop());
    assert!(
        std::pin::pin!(stream.result())
            .as_mut()
            .poll(&mut context)
            .is_pending()
    );
}

/// Retain the agent's ordered observations and both authoritative final-message handles.
fn observe_agent(
    session: &AgentSession,
    events: &Rc<RefCell<Vec<Value>>>,
    finals: &Rc<RefCell<Vec<SharedAssistantMessage>>>,
) {
    let (events, finals) = (events.clone(), finals.clone());
    session.agent().subscribe(Rc::new(move |event, _| {
        let event = event.read().unwrap();
        events
            .borrow_mut()
            .push(serde_json::to_value(&*event).unwrap());
        match &*event {
            AgentEvent::MessageEnd {
                message: AgentMessage::Assistant(message),
            }
            | AgentEvent::TurnEnd { message, .. } => finals.borrow_mut().push(message.clone()),
            _ => (),
        }
        Box::pin(async { Ok(()) })
    }));
}

/// Literal event order proves the actual agent loop consumed the guest stream without draining its result.
fn check(events: &[Value], finals: &[SharedAssistantMessage], result: &SharedAssistantMessage) {
    assert_eq!(
        events
            .iter()
            .map(|event| event["type"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "agent_start",
            "turn_start",
            "message_start",
            "message_end",
            "message_start",
            "message_update",
            "message_end",
            "turn_end",
            "agent_end",
        ]
    );
    assert_eq!(events[5]["assistantMessageEvent"]["type"], "text_delta");
    assert_eq!(events[5]["assistantMessageEvent"]["delta"], "ordered delta");
    assert_eq!(events[6]["message"]["content"][0]["text"], "false");
    assert_eq!(finals.len(), 2);
    for message in finals {
        assert!(Arc::ptr_eq(message, result));
    }
    assert_eq!(
        result.read().unwrap().stop_reason,
        maestro_app::StopReason::Stop
    );
}

/// Install native hooks on the actual agent before it constructs its stream options.
fn native_hooks(
    session: &AgentSession,
    entered: mpsc::UnboundedSender<(&'static str, oneshot::Sender<()>)>,
) {
    let payload = entered.clone();
    *session.agent().on_payload.write().unwrap() = Some(Rc::new(move |mut input, model| {
        assert_eq!(model.id, "registered-case");
        assert_eq!(
            input.payload,
            json!({"original":true,"unchanged":"neighbor"})
        );
        let entered = payload.clone();
        Box::pin(async move {
            gate(&entered, "payload").await;
            input.payload = json!(false);
            Ok(input)
        })
    }));
    let response = entered;
    *session.agent().on_response.write().unwrap() = Some(Rc::new(move |response_value, model| {
        assert_eq!(model.id, "registered-case");
        assert_eq!(response_value.status.to_bits(), 207.5_f64.to_bits());
        assert_eq!(response_value.headers["x-native"], "literal");
        let entered = response.clone();
        Box::pin(async move {
            gate(&entered, "response").await;
            Ok(())
        })
    }));
}
