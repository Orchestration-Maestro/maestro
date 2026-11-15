//! Generic callback and record fixtures for protocol registration.
use maestro_models::SharedHandle;
use std::sync::Arc;
pub(crate) fn model(api: &str) -> maestro_models::records::types::Model {
    serde_json::from_value(serde_json::json!({"id":"custom-model","name":"Custom","api":api,"provider":"custom","baseUrl":"https://fixture.invalid","reasoning":false,"input":["text"],"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":100,"maxTokens":10})).unwrap()
}

pub(crate) fn context() -> maestro_models::records::types::Context {
    maestro_models::records::types::Context {
        system_prompt: Some("system".into()),
        messages: vec![],
        tools: None,
    }
}

pub(crate) fn final_stream(
    timestamp: f64,
) -> maestro_models::records::types::AssistantMessageEventStream {
    use maestro_models::records::types::{
        AssistantMessageEvent, AssistantMessageEventStream, DoneReason,
    };
    let message = Arc::new(std::sync::RwLock::new(serde_json::from_value(serde_json::json!({"role":"assistant","content":[],"api":"custom","provider":"custom","model":"m","usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},"stopReason":"stop","timestamp":timestamp})).unwrap()));
    let stream = AssistantMessageEventStream::new();
    stream.push(AssistantMessageEvent::Done {
        reason: DoneReason::Stop,
        message,
    });
    stream
}

pub(crate) fn ready<F: std::future::Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    match future
        .as_mut()
        .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
    {
        std::task::Poll::Ready(value) => value,
        std::task::Poll::Pending => panic!("expected ready"),
    }
}

pub(crate) fn provider(
    api: &str,
    timestamp: f64,
) -> maestro_models::records::api_registry::ApiProvider {
    maestro_models::records::api_registry::ApiProvider {
        api: api.into(),
        stream: SharedHandle::new(TimestampProvider(timestamp)),
        stream_simple: SharedHandle::new(TimestampProvider(timestamp + 1.0)),
    }
}

pub(crate) fn assert_timestamp(
    stream: &maestro_models::AssistantMessageEventStream,
    expected: f64,
) {
    assert_eq!(
        ready(stream.result()).read().unwrap().timestamp.to_bits(),
        expected.to_bits()
    );
}

/// Retained invocation owner for the controlled provider.
pub(crate) struct TimestampProvider(pub(crate) f64);
impl<E: maestro_models::ProviderExecution, O>
    maestro_models::Invoke<E, maestro_models::ProviderCall<E, O>> for TimestampProvider
{
    type Return =
        Result<maestro_models::AssistantMessageEventStream, maestro_models::DiagnosticErrorInfo>;
    fn invoke(&self, _call: maestro_models::ProviderCall<E, O>) -> Self::Return {
        Ok(final_stream(self.0))
    }
}
