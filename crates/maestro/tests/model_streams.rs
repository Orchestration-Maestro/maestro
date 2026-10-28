//! The native agent consumes a compiled provider's retained stream through the SDK route.
use maestro_app::{
    AgentEvent, AgentMessage, AgentSession, AssistantMessageEventStream, PromptOptions,
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
        let session = super::support::session::component_session(
            &root,
            local.clone(),
            std::env::var("MAESTRO_CONTEXT_COMPONENT").expect("nextest author-component setup"),
            Some(maestro_app::NoTools::All),
            super::support::widgets(),
        )
        .await;
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
