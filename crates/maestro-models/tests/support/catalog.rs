#![allow(dead_code)]
use super::conformance::{Gate, done};
use maestro_models::*;
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

pub fn entry(provider: &str, id: &str, operation: &str) -> Model {
    Model::custom(
        ModelIdentity {
            provider: provider.into(),
            model: id.into(),
            operation: operation.into(),
        },
        "opaque:protocol/path".into(),
        "opaque:endpoint/path".into(),
    )
}
pub fn models() -> Models {
    Models::new(Arc::new(|| 73))
}

pub struct Getter {
    pub calls: AtomicUsize,
    pub result: Result<Vec<Model>, Failure>,
}
impl Getter {
    pub fn get(&self) -> Result<Vec<Model>, Failure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.result.clone()
    }
}

pub struct RecordingProvider {
    pub calls: Mutex<Vec<(Model, ProviderOptions)>>,
    pub body_gate: Option<Gate>,
    pub supports_chat: bool,
    pub description: ProviderDescription,
}
impl Default for RecordingProvider {
    fn default() -> Self {
        Self {
            calls: Default::default(),
            body_gate: None,
            supports_chat: true,
            description: Default::default(),
        }
    }
}
impl Provider for RecordingProvider {
    fn supports(&self, operation: &str) -> bool {
        self.supports_chat && operation == "chat"
    }
    fn description(&self) -> ProviderDescription {
        self.description.clone()
    }
    fn stream(
        &self,
        model: Model,
        _: Context,
        options: ProviderOptions,
    ) -> Result<Box<dyn ProviderStream>, Failure> {
        self.calls.lock().unwrap().push((model, options));
        Ok(Box::new(RecordingStream {
            wait: self.body_gate.as_ref().map(Gate::wait),
            done: false,
        }))
    }
}
struct RecordingStream {
    wait: Option<Pin<Box<dyn Future<Output = ()> + Send>>>,
    done: bool,
}
impl ProviderStream for RecordingStream {
    fn next(&mut self) -> Pin<Box<dyn Future<Output = Option<ProviderUpdate>> + Send + '_>> {
        Box::pin(async move {
            if let Some(wait) = &mut self.wait {
                wait.await;
            }
            self.wait = None;
            if self.done {
                None
            } else {
                self.done = true;
                Some(done())
            }
        })
    }
}
