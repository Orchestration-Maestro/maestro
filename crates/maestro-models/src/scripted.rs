//! Credential-free queued explicit adapter updates.

use crate::{Context, Failure, Model, Provider, ProviderStream, ProviderUpdate};
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

struct State {
    responses: VecDeque<Vec<ProviderUpdate>>,
    calls: Vec<(Model, Context)>,
}

/// A credential-free adapter with queued explicit response updates.
/// Only this adapter owns its queue and request history; observations are cloned.
pub struct ScriptedProvider {
    state: Mutex<State>,
}

impl ScriptedProvider {
    /// Queue one sequence of updates for each future invocation.
    pub fn new(responses: Vec<Vec<ProviderUpdate>>) -> Self {
        Self {
            state: Mutex::new(State {
                responses: responses.into(),
                calls: Vec::new(),
            }),
        }
    }

    /// Return an owned snapshot of invoked models and contexts in call order.
    pub fn calls(&self) -> Vec<(Model, Context)> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .calls
            .clone()
    }

    /// Return the number of response sequences not yet dispatched.
    pub fn pending(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .responses
            .len()
    }
}

impl Provider for ScriptedProvider {
    fn supports(&self, operation: &str) -> bool {
        operation == "chat"
    }

    fn stream(&self, model: Model, context: Context) -> Result<Box<dyn ProviderStream>, Failure> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.calls.push((model, context));
        let updates = state
            .responses
            .pop_front()
            .ok_or(Failure::ScriptExhausted)?;
        Ok(Box::new(ScriptedStream {
            updates: updates.into(),
        }))
    }
}

struct ScriptedStream {
    updates: VecDeque<ProviderUpdate>,
}

impl ProviderStream for ScriptedStream {
    fn next(&mut self) -> Pin<Box<dyn Future<Output = Option<ProviderUpdate>> + Send + '_>> {
        Box::pin(async move { self.updates.pop_front() })
    }
}
