//! Caller-owned execution facilities for lifecycle observations.
use maestro_agent::{Agent, AgentOptions, AgentSpawner, PromptInput};
use maestro_models::{BoxFuture, DiagnosticErrorInfo};
use std::sync::{Arc, Mutex};

/// Queued submitted work; tests explicitly drive and join each task.
#[derive(Default)]
pub struct Tasks(pub Mutex<Vec<BoxFuture<()>>>);
impl Tasks {
    /// Admission callback retaining submitted tasks without executing them.
    ///
    /// # Panics
    /// Panics if the controlled task queue is poisoned.
    pub fn spawner(self: &Arc<Self>) -> AgentSpawner {
        let tasks = Arc::clone(self);
        Arc::new(move |task| tasks.0.lock().unwrap().push(task))
    }
    /// Take one admitted task for controlled execution.
    ///
    /// # Panics
    /// Panics if no task was admitted or the queue is poisoned.
    pub fn take(&self) -> BoxFuture<()> {
        self.0.lock().unwrap().pop().unwrap()
    }
}
/// Construct an inert Agent with a controlled response and queued execution.
#[must_use]
pub fn agent() -> (Arc<Agent>, Arc<Tasks>) {
    let tasks = Arc::new(Tasks::default());
    let agent = Arc::new(Agent::new(AgentOptions {
        spawn: Some(tasks.spawner()),
        stream_fn: super::responses(vec![super::assistant("done")]).stream_fn,
        ..Default::default()
    }));
    (agent, tasks)
}
/// A text-only prompt.
#[must_use]
pub fn input(text: &str) -> PromptInput {
    PromptInput::Text {
        text: text.into(),
        images: vec![],
    }
}
/// Caller diagnostics preserved by the failure path.
#[must_use]
pub fn failure(text: &str) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        name: Some("Error".into()),
        message: text.into(),
        code: None,
        stack: None,
    }
}
/// The operation-specific busy diagnostic.
pub const BUSY: &str = "Agent is already processing a prompt. Use steer() or followUp() to queue messages, or wait for completion.";
