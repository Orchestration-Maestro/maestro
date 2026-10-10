//! Awaited sequential conversation turns.
use crate::{
    AgentContext, AgentEvent, AgentLoopConfig, AgentMessage, CustomAgentMessages, StreamFn,
};
use maestro_models::{BoxFuture, Cancellation, DiagnosticErrorInfo};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

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
/// Sequential tool execution.
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
    prompt(prompts, context, config, emit, options).await
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
    continue_loop(context, config, emit, options).await
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
    prompt(prompts, context, config, emit, options).await
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
    continue_loop(context, config, emit, options).await
}
/// Emit prompt observations over a copied outer history, returning new entries.
async fn prompt<C: CustomAgentMessages + 'static>(
    prompts: Vec<AgentMessage<C>>,
    mut context: AgentContext<C>,
    config: &AgentLoopConfig<C>,
    emit: AgentEventSink<C>,
    options: AgentLoopOptions,
) -> Result<Vec<AgentMessage<C>>, DiagnosticErrorInfo> {
    let history = read(&context.messages)
        .iter()
        .chain(&prompts)
        .cloned()
        .collect();
    context.messages = Arc::new(RwLock::new(history));
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
/// Continue through the supplied history without replaying its existing tail.
async fn continue_loop<C: CustomAgentMessages + 'static>(
    context: AgentContext<C>,
    config: &AgentLoopConfig<C>,
    emit: AgentEventSink<C>,
    options: AgentLoopOptions,
) -> Result<Vec<AgentMessage<C>>, DiagnosticErrorInfo> {
    match read(&context.messages).last() {
        None => return Err(failure("Cannot continue: no messages in context")),
        Some(AgentMessage::Assistant(_)) => {
            return Err(failure("Cannot continue from message role: assistant"));
        }
        _ => (),
    }
    emit(AgentEvent::AgentStart).await?;
    emit(AgentEvent::TurnStart).await?;
    Driver {
        context,
        config,
        emit,
        options,
    }
    .run(Vec::new())
    .await
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
    /// Complete assistant turns until no tool continuation remains.
    async fn run(
        &self,
        mut messages: Vec<AgentMessage<C>>,
    ) -> Result<Vec<AgentMessage<C>>, DiagnosticErrorInfo> {
        let mut first = true;
        loop {
            if !first {
                (self.emit)(AgentEvent::TurnStart).await?;
            }
            first = false;
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
            for result in &results {
                let message = AgentMessage::ToolResult(Arc::clone(result));
                write(&self.context.messages).push(message.clone());
                messages.push(message);
            }
            (self.emit)(AgentEvent::TurnEnd {
                message: response,
                tool_results: results.iter().map(|result| read(result).clone()).collect(),
            })
            .await?;
            if !again {
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

/// Construct a loop-owned diagnostic without changing caller diagnostics.
fn failure(message: impl Into<String>) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        name: Some("Error".into()),
        message: message.into(),
        stack: None,
        code: None,
    }
}
