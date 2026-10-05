//! Credential-free FIFO response scripts with owned request observations.

use crate::{Context, Failure, Model, Provider, ProviderOptions, ProviderStream, ProviderUpdate};
use std::{collections::VecDeque, future::Future, pin::Pin, sync::Mutex};

/// An independent observation of one adapter dispatch.
#[derive(Clone)]
pub struct ScriptedCall {
    /// Requested model.
    pub model: Model,
    /// Owned request context.
    pub context: Context,
    /// Request options; cancellation deliberately shares its signal.
    pub options: ProviderOptions,
    /// One-based dispatch index, including failures and exhaustion.
    pub call_index: usize,
}

/// A one-shot asynchronous request-inspecting script factory.
pub type ScriptFactory = Box<
    dyn FnOnce(
            ScriptedCall,
        )
            -> Pin<Box<dyn Future<Output = Result<Vec<ScriptStep>, Failure>> + Send + 'static>>
        + Send
        + 'static,
>;

/// One queued request response.
pub enum Script {
    /// Explicit source actions.
    Steps(Vec<ScriptStep>),
    /// Lazy asynchronous response generation.
    Factory(ScriptFactory),
    /// Immediate typed setup failure.
    SetupFailure(Failure),
}

/// One explicit update or controlled asynchronous wait.
pub enum ScriptStep {
    /// Adapter update to normalize.
    Update(ProviderUpdate),
    /// Caller-controlled wait with no elapsed-time pacing.
    Wait(Pin<Box<dyn Future<Output = ()> + Send + 'static>>),
}

struct State {
    responses: VecDeque<Script>,
    calls: Vec<ScriptedCall>,
}

/// A replaceable chat adapter driven by queued explicit response scripts.
pub struct ScriptedProvider {
    state: Mutex<State>,
}

impl ScriptedProvider {
    /// Queue one response for each future dispatch; exhaustion never replays.
    pub fn new(scripts: Vec<Script>) -> Self {
        Self {
            state: Mutex::new(State {
                responses: scripts.into(),
                calls: Vec::new(),
            }),
        }
    }
    /// Clone independent request records in dispatch order.
    pub fn calls(&self) -> Vec<ScriptedCall> {
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .calls
            .clone()
    }
    /// Count queued responses, excluding the active response.
    pub fn pending(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .responses
            .len()
    }
}

impl Provider for ScriptedProvider {
    fn supports(&self, operation: &str) -> bool {
        operation == "chat"
    }
    fn stream(
        &self,
        model: Model,
        context: Context,
        options: ProviderOptions,
    ) -> Result<Box<dyn ProviderStream>, Failure> {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        let call = ScriptedCall {
            model,
            context,
            options,
            call_index: state.calls.len() + 1,
        };
        state.calls.push(call.clone());
        let script = state
            .responses
            .pop_front()
            .ok_or(Failure::ScriptExhausted)?;
        drop(state);
        match script {
            Script::Steps(steps) => Ok(Box::new(ScriptedStream {
                steps: steps.into(),
                factory: None,
                factory_future: None,
            })),
            Script::SetupFailure(failure) => Err(failure),
            Script::Factory(factory) => Ok(Box::new(ScriptedStream {
                steps: VecDeque::new(),
                factory: Some((factory, call)),
                factory_future: None,
            })),
        }
    }
}

type FactoryFuture = Pin<Box<dyn Future<Output = Result<Vec<ScriptStep>, Failure>> + Send>>;
struct ScriptedStream {
    steps: VecDeque<ScriptStep>,
    factory: Option<(ScriptFactory, ScriptedCall)>,
    factory_future: Option<FactoryFuture>,
}
impl ProviderStream for ScriptedStream {
    fn next(&mut self) -> Pin<Box<dyn Future<Output = Option<ProviderUpdate>> + Send + '_>> {
        Box::pin(async move {
            if let Some((factory, call)) = self.factory.take() {
                self.factory_future = Some(factory(call));
            }
            if let Some(future) = self.factory_future.as_mut() {
                let result = future.await;
                self.factory_future = None;
                match result {
                    Ok(steps) => self.steps = steps.into(),
                    Err(failure) => return Some(ProviderUpdate::Error { failure }),
                }
            }
            loop {
                match self.steps.front_mut() {
                    Some(ScriptStep::Wait(wait)) => {
                        wait.await;
                        self.steps.pop_front();
                    }
                    Some(ScriptStep::Update(_)) => {
                        if let Some(ScriptStep::Update(update)) = self.steps.pop_front() {
                            return Some(update);
                        }
                    }
                    None => return None,
                }
            }
        })
    }
}
