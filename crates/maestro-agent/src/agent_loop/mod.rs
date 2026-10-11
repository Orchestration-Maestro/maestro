//! Awaited and observable conversation turns.
use crate::{
    AgentContext, AgentEvent, AgentLoopConfig, AgentMessage, AgentSpawner, CustomAgentMessages,
    StreamFn,
};
use maestro_models::{BoxFuture, Cancellation, DiagnosticErrorInfo, EventStream};
use std::io::Write;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Submit owned prompt work and return its observable stream without awaiting it.
#[cfg(not(target_arch = "wasm32"))]
pub fn agent_loop<C: CustomAgentMessages + Send + Sync + 'static>(
    prompts: Vec<AgentMessage<C>>,
    context: AgentContext<C>,
    config: Arc<AgentLoopConfig<C>>,
    options: AgentLoopOptions,
    spawn: AgentSpawner,
) -> EventStream<AgentEvent<C>, Vec<AgentMessage<C>>> {
    submit(Some(prompts), context, config, options, spawn)
}
/// Submit continuation without replaying the existing tail.
///
/// # Errors
/// Rejects empty or assistant tails before submitting work.
#[cfg(not(target_arch = "wasm32"))]
pub fn agent_loop_continue<C: CustomAgentMessages + Send + Sync + 'static>(
    context: AgentContext<C>,
    config: Arc<AgentLoopConfig<C>>,
    options: AgentLoopOptions,
    spawn: AgentSpawner,
) -> Result<EventStream<AgentEvent<C>, Vec<AgentMessage<C>>>, DiagnosticErrorInfo> {
    check_continuation(&context)?;
    Ok(submit(None, context, config, options, spawn))
}
/// Submit owned prompt work and return its observable stream without awaiting it.
#[cfg(target_arch = "wasm32")]
pub fn agent_loop<C: CustomAgentMessages + 'static>(
    prompts: Vec<AgentMessage<C>>,
    context: AgentContext<C>,
    config: Arc<AgentLoopConfig<C>>,
    options: AgentLoopOptions,
    spawn: AgentSpawner,
) -> EventStream<AgentEvent<C>, Vec<AgentMessage<C>>> {
    submit(Some(prompts), context, config, options, spawn)
}
/// Submit continuation without replaying the existing tail.
///
/// # Errors
/// Rejects empty or assistant tails before submitting work.
#[cfg(target_arch = "wasm32")]
pub fn agent_loop_continue<C: CustomAgentMessages + 'static>(
    context: AgentContext<C>,
    config: Arc<AgentLoopConfig<C>>,
    options: AgentLoopOptions,
    spawn: AgentSpawner,
) -> Result<EventStream<AgentEvent<C>, Vec<AgentMessage<C>>>, DiagnosticErrorInfo> {
    check_continuation(&context)?;
    Ok(submit(None, context, config, options, spawn))
}
/// Construct the terminal-selected observation stream.
fn create_agent_stream<C: CustomAgentMessages + crate::agent::Native + 'static>()
-> EventStream<AgentEvent<C>, Vec<AgentMessage<C>>> {
    EventStream::new(
        |event| matches!(event, AgentEvent::AgentEnd { .. }),
        |event| match event {
            AgentEvent::AgentEnd { messages } => messages.clone(),
            _ => Vec::new(),
        },
    )
}
/// Submit one awaited operation, retaining production independently of readers.
fn submit<C: CustomAgentMessages + crate::agent::Native + 'static>(
    prompts: Option<Vec<AgentMessage<C>>>,
    mut context: AgentContext<C>,
    config: Arc<AgentLoopConfig<C>>,
    options: AgentLoopOptions,
    spawn: AgentSpawner,
) -> EventStream<AgentEvent<C>, Vec<AgentMessage<C>>> {
    if let Some(prompts) = &prompts {
        context = prepare_prompt(prompts, context);
    }
    let prompts = prompts.unwrap_or_default();
    let stream = create_agent_stream();
    let producer = stream.clone();
    let sink = stream.clone();
    let emit: AgentEventSink<C> = Arc::new(move |event| {
        sink.push(event);
        Box::pin(std::future::ready(Ok(())))
    });
    spawn(Box::pin(async move {
        let result = prepared_loop(prompts, context, &config, emit, options).await;
        match result {
            Ok(messages) => producer.end(Some(messages)),
            Err(error) => {
                let _ = writeln!(std::io::stderr(), "{}", error.message);
            }
        }
    }));
    drop(spawn);
    stream
}
/// Check continuation admission without converting custom entries.
fn check_continuation<C: CustomAgentMessages>(
    context: &AgentContext<C>,
) -> Result<(), DiagnosticErrorInfo> {
    let diagnostic = match read(&context.messages).last() {
        None => Some("Cannot continue: no messages in context"),
        Some(AgentMessage::Assistant(_)) => Some("Cannot continue from message role: assistant"),
        _ => None,
    };
    diagnostic.map_or(Ok(()), |message| Err(failure(message)))
}
/// Await one conversation observation.
#[cfg(not(target_arch = "wasm32"))]
pub type AgentEventSink<C = std::convert::Infallible> =
    Arc<dyn Fn(AgentEvent<C>) -> BoxFuture<Result<(), DiagnosticErrorInfo>> + Send + Sync>;
/// Await one browser-local conversation observation.
#[cfg(target_arch = "wasm32")]
pub type AgentEventSink<C = std::convert::Infallible> =
    Arc<dyn Fn(AgentEvent<C>) -> BoxFuture<Result<(), DiagnosticErrorInfo>>>;
/// Request and assistant-event handling.
mod assistant;
/// Concurrent settlement of admitted tool progress.
mod progress;
/// Tool execution.
mod tools;
/// Optional invocation signal and replacement model stream.
#[derive(Default)]
pub struct AgentLoopOptions {
    /// Signal replacing the configured request signal, including when absent.
    pub signal: Option<Cancellation>,
    /// Replacement stream construction; absence selects the model adapter.
    pub stream_fn: Option<StreamFn>,
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
/// Run prompts over a copied outer history, retaining shared entries.
///
/// # Errors
/// Returns request callback, model setup or awaited observation failures.
#[cfg(not(target_arch = "wasm32"))]
pub async fn run_agent_loop<C: CustomAgentMessages + Send + Sync + 'static>(
    prompts: Vec<AgentMessage<C>>,
    context: AgentContext<C>,
    config: &AgentLoopConfig<C>,
    emit: AgentEventSink<C>,
    options: AgentLoopOptions,
) -> Result<Vec<AgentMessage<C>>, DiagnosticErrorInfo> {
    let context = prepare_prompt(&prompts, context);
    prepared_loop(prompts, context, config, emit, options).await
}
/// Continue through retained history without replaying its tail.
///
/// # Errors
/// Rejects empty or assistant tails before observations; otherwise propagates
/// request callback, model setup or awaited observation failures.
#[cfg(not(target_arch = "wasm32"))]
pub async fn run_agent_loop_continue<C: CustomAgentMessages + Send + Sync + 'static>(
    context: AgentContext<C>,
    config: &AgentLoopConfig<C>,
    emit: AgentEventSink<C>,
    options: AgentLoopOptions,
) -> Result<Vec<AgentMessage<C>>, DiagnosticErrorInfo> {
    check_continuation(&context)?;
    prepared_loop(Vec::new(), context, config, emit, options).await
}
/// Run prompts over a copied outer history, retaining shared entries.
///
/// # Errors
/// Returns request callback, model setup or awaited observation failures.
#[cfg(target_arch = "wasm32")]
pub async fn run_agent_loop<C: CustomAgentMessages + 'static>(
    prompts: Vec<AgentMessage<C>>,
    context: AgentContext<C>,
    config: &AgentLoopConfig<C>,
    emit: AgentEventSink<C>,
    options: AgentLoopOptions,
) -> Result<Vec<AgentMessage<C>>, DiagnosticErrorInfo> {
    let context = prepare_prompt(&prompts, context);
    prepared_loop(prompts, context, config, emit, options).await
}
/// Continue through retained history without replaying its tail.
///
/// # Errors
/// Rejects empty or assistant tails before observations; otherwise propagates
/// request callback, model setup or awaited observation failures.
#[cfg(target_arch = "wasm32")]
pub async fn run_agent_loop_continue<C: CustomAgentMessages + 'static>(
    context: AgentContext<C>,
    config: &AgentLoopConfig<C>,
    emit: AgentEventSink<C>,
    options: AgentLoopOptions,
) -> Result<Vec<AgentMessage<C>>, DiagnosticErrorInfo> {
    check_continuation(&context)?;
    prepared_loop(Vec::new(), context, config, emit, options).await
}
/// Execute prepared prompt or admitted continuation observations.
async fn prepared_loop<C: CustomAgentMessages + 'static>(
    prompts: Vec<AgentMessage<C>>,
    context: AgentContext<C>,
    config: &AgentLoopConfig<C>,
    emit: AgentEventSink<C>,
    options: AgentLoopOptions,
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
    let driver = Driver {
        context,
        config,
        emit,
        options,
    };
    driver.run(prompts).await
}
/// Capture the prompt history's outer collection without copying its entries.
fn prepare_prompt<C: CustomAgentMessages>(
    prompts: &[AgentMessage<C>],
    mut context: AgentContext<C>,
) -> AgentContext<C> {
    let history = read(&context.messages)
        .iter()
        .chain(prompts)
        .cloned()
        .collect();
    context.messages = Arc::new(RwLock::new(history));
    context
}
/// Inputs retained for the operation's turn progression.
struct Driver<'a, C: CustomAgentMessages> {
    /// Instruction, history and tool handles.
    context: AgentContext<C>,
    /// Caller request configuration.
    config: &'a AgentLoopConfig<C>,
    /// Awaited observation callback.
    emit: AgentEventSink<C>,
    /// Invocation overrides.
    options: AgentLoopOptions,
}
impl<C: CustomAgentMessages + 'static> Driver<'_, C> {
    /// Complete turns until controls and ordinary continuation are exhausted.
    async fn run(
        &self,
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
            self.complete_turn(&response, &results, &mut messages)
                .await?;
            if failed || self.stop_after_turn(&response, &results, &messages).await? {
                break;
            }
            pending = self.next_messages(again).await?;
            if !again && pending.is_empty() {
                break;
            }
        }
        (self.emit)(AgentEvent::AgentEnd {
            messages: messages.clone(),
        })
        .await?;
        Ok(messages)
    }
}

/// Queue injection and completed-turn decisions.
mod controls;

/// Construct a loop-owned diagnostic without changing caller diagnostics.
fn failure(message: impl Into<String>) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        name: Some("Error".into()),
        message: message.into(),
        stack: None,
        code: None,
    }
}
