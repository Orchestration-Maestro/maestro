//! A registered completion adapter answering with scripted summary text and usage.
use maestro_models::{
    ApiProvider, AssistantMessage, AssistantMessageEventStream, DiagnosticErrorInfo, Model,
};
use maestro_session::SummaryOptions;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, RwLock};

/// Usage reporting `input` and `output` tokens at `cost` in total.
pub fn usage(input: u64, output: u64, cost: f64) -> Value {
    json!({"input": input, "output": output, "cacheRead": 0, "cacheWrite": 0,
        "totalTokens": input + output,
        "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": cost}})
}

/// Scripted answers in request order, each a summary text and its usage.
pub struct ScriptedSummary {
    /// Isolated registry identity.
    api: String,
    /// Answers not yet consumed.
    answers: Arc<Mutex<VecDeque<(String, Value)>>>,
}

impl Drop for ScriptedSummary {
    fn drop(&mut self) {
        maestro_models::unregister_api_providers(&self.api);
    }
}

impl ScriptedSummary {
    /// Register a provider answering the next requests with `answers`, and a model for it.
    pub fn register(answers: Vec<(&str, Value)>) -> (Self, Model) {
        let api = format!("scripted-summary-{}", uuid::Uuid::new_v4());
        let queue = Arc::new(Mutex::new(
            answers
                .into_iter()
                .map(|(text, usage)| (text.to_owned(), usage))
                .collect(),
        ));
        let provider = Arc::new(Provider(Arc::clone(&queue)));
        maestro_models::register_api_provider(
            ApiProvider {
                api: api.clone(),
                stream: Arc::new(Unexpected),
                stream_simple: provider,
            },
            Some(api.clone()),
        );
        let model: Model = serde_json::from_value(json!({
            "id": "scripted", "name": "Scripted", "api": api, "provider": "scripted",
            "baseUrl": "https://invalid.example", "reasoning": false, "input": ["text"],
            "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0},
            "contextWindow": 200_000, "maxTokens": 8192
        }))
        .unwrap();
        (
            Self {
                api,
                answers: queue,
            },
            model,
        )
    }

    /// Answers the provider has not consumed.
    pub fn remaining(&self) -> usize {
        self.answers.lock().unwrap().len()
    }
}

/// Summary controls with explicit empty authentication.
pub fn options() -> SummaryOptions {
    SummaryOptions {
        api_key: String::new(),
        headers: None,
        signal: None,
        custom_instructions: None,
        thinking_level: None,
    }
}

/// The scripted completion owner.
struct Provider(Arc<Mutex<VecDeque<(String, Value)>>>);

impl<E: maestro_models::ProviderExecution>
    maestro_models::Invoke<
        E,
        maestro_models::ProviderCall<E, maestro_models::SimpleStreamOptions<E>>,
    > for Provider
{
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(
        &self,
        call: maestro_models::ProviderCall<E, maestro_models::SimpleStreamOptions<E>>,
    ) -> Self::Return {
        let (text, usage) = self.0.lock().unwrap().pop_front().unwrap();
        let response: AssistantMessage = serde_json::from_value(json!({
            "role": "assistant", "content": [{"type": "text", "text": text}],
            "api": call.model.api, "provider": "scripted", "model": "scripted",
            "usage": usage, "stopReason": "stop", "timestamp": 1
        }))
        .unwrap();
        let stream = AssistantMessageEventStream::new();
        stream.end(Some(Arc::new(RwLock::new(response))));
        Ok(stream)
    }
}

/// A raw dispatch is never selected.
struct Unexpected;

impl<E: maestro_models::ProviderExecution, O>
    maestro_models::Invoke<E, maestro_models::ProviderCall<E, O>> for Unexpected
{
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(&self, _call: maestro_models::ProviderCall<E, O>) -> Self::Return {
        panic!("raw dispatch selected")
    }
}
