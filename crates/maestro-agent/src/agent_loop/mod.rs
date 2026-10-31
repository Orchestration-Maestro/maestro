//! Awaited and observable conversation turns.
use crate::types::{AgentContextRecord, AgentExecution, AgentLoopConfigRecord, Threaded};

use crate::{AgentEvent, AgentMessage, AgentSpawner, CustomAgentMessages, StreamFn};
use maestro_models::{Cancellation, DiagnosticErrorInfo, EventStream};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Caller-owned stream submission.
mod submission;
pub(crate) use submission::Submitted;
pub use submission::local;
pub use submission::{agent_loop, agent_loop_continue, run_agent_loop, run_agent_loop_continue};
/// Await one conversation observation.
pub type AgentEventSink<C = crate::NoCustomMessages, E = Threaded> =
    <E as maestro_models::Execution>::Shared<<E as AgentExecution>::Sink<C>>;
/// Request and assistant-event handling.
mod assistant;
pub use assistant::tool_definitions;
/// Concurrent settlement of admitted tool progress.
pub(crate) mod progress;
/// Tool execution.
mod tools;
/// Optional invocation signal and replacement model stream.
#[derive(Default)]
pub struct AgentLoopOptionsRecord<E: AgentExecution = Threaded> {
    /// Signal replacing the configured request signal, including when absent.
    pub signal: Option<Cancellation>,
    /// Replacement stream construction; absence selects the model adapter.
    pub stream_fn: Option<StreamFn<E>>,
}
/// Read retained state even after an unrelated panic poisoned its lock.
fn read<T>(value: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    value
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
/// Write retained state even after an unrelated panic poisoned its lock.
fn write<T>(value: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    value
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
/// Apply prompt copying or continuation admission before shared execution.
pub(crate) async fn run_for<C: CustomAgentMessages + 'static, E: crate::ExecutionFor<C>>(
    prompts: Option<Vec<AgentMessage<C>>>,
    mut context: AgentContextRecord<C, E>,
    config: &AgentLoopConfigRecord<C, E>,
    emit: AgentEventSink<C, E>,
    options: CapturedOptions<E>,
) -> Result<Vec<AgentMessage<C>>, DiagnosticErrorInfo> {
    let prompts = if let Some(prompts) = prompts {
        context = prepare_prompt(&prompts, context);
        prompts
    } else {
        check_continuation(&context)?;
        Vec::new()
    };
    prepared_loop(prompts, context, config, emit, options).await
}
/// Execute prepared prompt or admitted continuation observations.
async fn prepared_loop<C: CustomAgentMessages + 'static, E: crate::ExecutionFor<C>>(
    prompts: Vec<AgentMessage<C>>,
    context: AgentContextRecord<C, E>,
    config: &AgentLoopConfigRecord<C, E>,
    emit: AgentEventSink<C, E>,
    options: CapturedOptions<E>,
) -> Result<Vec<AgentMessage<C>>, DiagnosticErrorInfo> {
    emit(AgentEvent::AgentStart).await?;
    emit(AgentEvent::TurnStart).await?;
    for prompt in &prompts {
        emit(AgentEvent::MessageStart {
            message: prompt.clone(),
        })
        .await?;
        emit(AgentEvent::MessageEnd {
            message: prompt.clone(),
        })
        .await?;
    }
    let mut driver = Driver {
        model: Arc::clone(&config.model),
        reasoning: config.options.reasoning,
        context,
        config,
        emit,
        options,
    };
    driver.run(prompts).await
}
/// Capture the prompt history's outer collection without copying its entries.
fn prepare_prompt<C: CustomAgentMessages, E: AgentExecution>(
    prompts: &[AgentMessage<C>],
    mut context: AgentContextRecord<C, E>,
) -> AgentContextRecord<C, E> {
    let history = read(&context.messages)
        .iter()
        .chain(prompts)
        .cloned()
        .collect();
    context.messages = Arc::new(RwLock::new(history));
    context
}
/// Inputs retained for the operation's turn progression.
struct Driver<'a, C: CustomAgentMessages, E: AgentExecution> {
    /// Instruction, history and tool handles.
    context: AgentContextRecord<C, E>,
    /// Model for the next request.
    model: Arc<RwLock<maestro_models::Model>>,
    /// Thinking level for the next request.
    reasoning: Option<maestro_models::ThinkingLevel>,
    /// Caller request configuration.
    config: &'a AgentLoopConfigRecord<C, E>,
    /// Awaited observation callback.
    emit: AgentEventSink<C, E>,
    /// Invocation overrides.
    options: CapturedOptions<E>,
}
/// One completed turn: its response, its tool results and what comes next.
struct FinishedTurn {
    /// The assistant's response.
    response: maestro_models::SharedAssistantMessage,
    /// The turn's tool results.
    results: Vec<Arc<RwLock<maestro_models::ToolResultMessage>>>,
    /// Whether the response ended in an error or an abort.
    failed: bool,
    /// Whether the tools asked for another turn.
    again: bool,
}
impl<C: CustomAgentMessages + 'static, E: crate::ExecutionFor<C>> Driver<'_, C, E> {
    /// Complete turns until controls and ordinary continuation are exhausted.
    async fn run(
        &mut self,
        mut messages: Vec<AgentMessage<C>>,
    ) -> Result<Vec<AgentMessage<C>>, DiagnosticErrorInfo> {
        let mut first = true;
        let mut pending = self
            .poll_messages(self.config.get_steering_messages.as_ref())
            .await?;
        loop {
            if !first {
                (self.emit)(AgentEvent::TurnStart).await?;
            }
            first = false;
            self.inject_messages(std::mem::take(&mut pending), &mut messages)
                .await?;
            let turn = self.turn(&mut messages).await?;
            if turn.failed
                || self
                    .stop_after_turn(&turn.response, &turn.results, &messages)
                    .await?
            {
                break;
            }
            pending = self.next_messages(turn.again).await?;
            if !turn.again && pending.is_empty() {
                break;
            }
        }
        (self.emit)(AgentEvent::AgentEnd {
            messages: messages.clone(),
        })
        .await?;
        Ok(messages)
    }
    /// One request, its tool calls and its end; a normal turn then prepares the next one.
    async fn turn(
        &mut self,
        messages: &mut Vec<AgentMessage<C>>,
    ) -> Result<FinishedTurn, DiagnosticErrorInfo> {
        let response = self.assistant().await?;
        messages.push(AgentMessage::Assistant(Arc::clone(&response)));
        let failed = matches!(
            read(&response).stop_reason,
            maestro_models::StopReason::Error | maestro_models::StopReason::Aborted
        );
        let (results, again) = if failed {
            (Vec::new(), false)
        } else {
            self.tools(&response).await?
        };
        self.complete_turn(&response, &results, messages).await?;
        if !failed {
            self.prepare_next_turn(&response, &results, messages)
                .await?;
        }
        Ok(FinishedTurn {
            response,
            results,
            failed,
            again,
        })
    }
}

/// Queue injection and completed-turn decisions.
mod controls;

/// Construct a loop-owned diagnostic without changing caller diagnostics.
fn failure(message: impl Into<String>) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo::error(message)
}

/// Thread-safe default representation of the `AgentLoopOptions` record.
pub type AgentLoopOptions = AgentLoopOptionsRecord<Threaded>;
/// Awaited local invocation with admission required only for registered dispatch.
///
/// Registered local calls cannot omit their task facility:
///
/// ```compile_fail
/// use maestro_agent::agent_loop::LocalAgentLoopOptions;
/// fn local_registered_dispatch_requires_submission() {
///     let _options = LocalAgentLoopOptions::RegisteredDispatch { signal: None };
/// }
/// ```
pub enum LocalAgentLoopOptions {
    /// Invoke the registered model through the caller's task facility.
    RegisteredDispatch {
        /// Signal replacing the configured request signal.
        signal: Option<Cancellation>,
        /// Caller-owned provider task admission.
        spawn: AgentSpawner<crate::Local>,
    },
    /// Invoke a replacement stream without admitting a provider task.
    ReplacementStream {
        /// Signal replacing the configured request signal.
        signal: Option<Cancellation>,
        /// Caller-supplied stream construction.
        stream_fn: StreamFn<crate::Local>,
    },
}
/// Invocation overrides retained with their selected provider route.
pub(crate) struct CapturedOptions<E: AgentExecution> {
    /// Signal replacing the configured request signal.
    pub signal: Option<Cancellation>,
    /// Registered admission or the replacement callable.
    pub route: StreamRoute<E>,
}
/// Mutually exclusive registered and replacement invocation inputs.
pub(crate) enum StreamRoute<E: AgentExecution> {
    /// Original family-specific admission.
    Registered(E::Submission),
    /// Caller-owned stream construction.
    Replacement(StreamFn<E>),
}
impl<E: AgentExecution> CapturedOptions<E> {
    /// Capture overrides and admission once before executing the operation.
    pub fn new(options: AgentLoopOptionsRecord<E>, submission: E::Submission) -> Self {
        Self {
            signal: options.signal,
            route: match options.stream_fn {
                Some(stream) => StreamRoute::Replacement(stream),
                None => StreamRoute::Registered(submission),
            },
        }
    }
}
impl From<AgentLoopOptionsRecord> for CapturedOptions<Threaded> {
    fn from(options: AgentLoopOptionsRecord) -> Self {
        Self::new(options, None)
    }
}
impl From<LocalAgentLoopOptions> for CapturedOptions<crate::Local> {
    fn from(options: LocalAgentLoopOptions) -> Self {
        match options {
            LocalAgentLoopOptions::RegisteredDispatch { signal, spawn } => Self {
                signal,
                route: StreamRoute::Registered(crate::Local::provider_submission(spawn)),
            },
            LocalAgentLoopOptions::ReplacementStream { signal, stream_fn } => Self {
                signal,
                route: StreamRoute::Replacement(stream_fn),
            },
        }
    }
}

/// Check continuation admission without converting custom entries.
fn check_continuation<C: CustomAgentMessages, E: AgentExecution>(
    context: &AgentContextRecord<C, E>,
) -> Result<(), DiagnosticErrorInfo> {
    let diagnostic = match read(&context.messages).last() {
        None => Some("Cannot continue: no messages in context"),
        Some(AgentMessage::Assistant(_)) => Some("Cannot continue from message role: assistant"),
        _ => None,
    };
    diagnostic.map_or(Ok(()), |message| Err(failure(message)))
}
