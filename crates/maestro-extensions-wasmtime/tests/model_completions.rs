//! Eager compiled-guest completion observes results without consuming native events.
#![cfg(test)]
use maestro_models::*;
use std::sync::{Arc, Mutex, RwLock};

/// The real registered adapter keeps native aliases, not a second completion route.
#[derive(Default)]
struct Pending(Mutex<Vec<AssistantMessageEventStream>>);
impl<E: ProviderExecution, O> Invoke<E, ProviderCall<E, O>> for Pending {
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(&self, call: ProviderCall<E, O>) -> Self::Return {
        assert_eq!(call.model.id, "owned-model");
        assert_eq!(call.context.system_prompt.as_deref(), Some("owned input"));
        let stream = AssistantMessageEventStream::new();
        self.0.lock().unwrap().push(stream.clone());
        Ok(stream)
    }
}

#[test]
fn completion_starts_before_result_is_observed() {
    let pending = Arc::new(Pending::default());
    register_api_provider(
        ApiProvider {
            api: "component-completions".into(),
            stream: pending.clone(),
            stream_simple: pending.clone(),
        },
        Some("component-completions".into()),
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<Local>(
            std::rc::Rc::new(|_| panic!("no tails")),
            std::rc::Rc::new(|_| panic!("no errors")),
        );
        let (_owner, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        observe_completion(&runner, &pending).await;
        bus.clear();
    });
    unregister_api_providers("component-completions");
}

/// Check eager admission before producing a result, then observe without iteration.
async fn observe_completion(
    runner: &maestro_extensions::ExtensionRunner<Local>,
    pending: &Pending,
) {
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    assert_eq!(
        command(
            "model-completions start".into(),
            runner.create_command_context()
        )
        .await
        .unwrap_err()
        .to_string(),
        "started"
    );
    let streams = std::mem::take(&mut *pending.0.lock().unwrap());
    assert_eq!(
        streams.len(),
        2,
        "complete must invoke before its future is observed"
    );
    let messages = finish_native(&streams);
    let output = command(
        "model-completions observe".into(),
        runner.create_command_context(),
    )
    .await
    .unwrap_err()
    .to_string();
    let output: serde_json::Value =
        serde_json::from_str(&output).unwrap_or_else(|_| panic!("{output}"));
    assert_eq!(output[0]["content"][0]["text"], "raw final");
    assert_eq!(output[1]["content"][0]["text"], "simple final");
    for (stream, message) in streams.iter().zip(&messages) {
        assert_queued(stream, message).await;
    }
}

/// Produce literal native updates and terminal messages through the original stream aliases.
fn finish_native(streams: &[AssistantMessageEventStream]) -> Vec<SharedAssistantMessage> {
    let messages: Vec<_> = ["raw final", "simple final"]
        .into_iter()
        .map(|text| {
            Arc::new(RwLock::new(faux_assistant_message(
                text,
                FauxAssistantMessageOptions::default(),
            )))
        })
        .collect();
    for (stream, message) in streams.iter().zip(&messages) {
        stream.push(AssistantMessageEvent::Start {
            partial: message.clone(),
        });
        stream.push(AssistantMessageEvent::TextDelta {
            content_index: 0,
            delta: "owned delta".into(),
            partial: message.clone(),
        });
        stream.push(AssistantMessageEvent::Done {
            reason: DoneReason::Stop,
            message: message.clone(),
        });
    }
    messages
}

/// Completion observation must leave every native event and the original final owner intact.
async fn assert_queued(stream: &AssistantMessageEventStream, message: &SharedAssistantMessage) {
    assert!(
        matches!(
            stream.next().await,
            Some(AssistantMessageEvent::Start { .. })
        ),
        "completion must not drain events"
    );
    assert!(matches!(
        stream.next().await,
        Some(AssistantMessageEvent::TextDelta { .. })
    ));
    assert!(matches!(
        stream.next().await,
        Some(AssistantMessageEvent::Done { .. })
    ));
    assert_settled(stream, message).await;
}

/// The drained native stream retains the same independently observed final owner.
async fn assert_settled(stream: &AssistantMessageEventStream, message: &SharedAssistantMessage) {
    assert!(stream.next().await.is_none());
    assert!(Arc::ptr_eq(message, &stream.result().await));
    assert!(Arc::ptr_eq(message, &stream.result().await));
}
