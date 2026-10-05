//! Tool lifecycle depth behind one source-ordered batch boundary.
use crate::{
    AgentContext, AgentError, AgentEvent, AgentMessage, ToolResult, agent::Inner, events::emit,
};
use maestro_models::{
    AssistantContent, AssistantMessage, Cancellation, InputContent, Message, TextContent, ToolCall,
    ToolResultMessage, validate_tool_call,
};
use serde_json::{Map, Value};
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::task::JoinSet;

type Outcome = (ToolResult, bool);
type Prepared = (crate::Tool, Map<String, Value>);

pub(crate) async fn run(
    inner: Arc<Inner>,
    assistant: &AssistantMessage,
    cancellation: &Cancellation,
    additions: &mut Vec<AgentMessage>,
) -> Result<(Vec<ToolResultMessage>, bool), AgentError> {
    let context = inner.lock().context.clone();
    let calls: Vec<_> = assistant
        .content
        .iter()
        .filter_map(|c| match c {
            AssistantContent::ToolCall(call) if call.arguments().is_some() => Some(call.clone()),
            _ => None,
        })
        .collect();
    let sequential = inner.options.tool_execution == crate::ToolExecutionMode::Sequential
        || calls.iter().any(|call| {
            inner
                .options
                .tools
                .iter()
                .find(|tool| tool.declaration.name == call.name)
                .is_some_and(|tool| {
                    tool.execution_mode == Some(crate::ToolExecutionMode::Sequential)
                })
        });
    if sequential {
        let mut messages = vec![];
        let mut terminating = !calls.is_empty();
        for call in calls {
            start(&inner, &call, cancellation).await;
            let outcome = match preflight(&inner, assistant, &call, &context, cancellation).await {
                Ok(input) => {
                    execute(&inner, assistant, &call, &context, cancellation, input).await?
                }
                Err(reason) => error(reason),
            };
            end(&inner, &call, &outcome, cancellation).await;
            terminating &= outcome.0.terminate == Some(true);
            messages.push(record(&inner, call, outcome, cancellation, additions).await);
        }
        return Ok((messages, terminating));
    }
    let mut outcomes = vec![None; calls.len()];
    let mut prepared = vec![];
    for (index, call) in calls.iter().enumerate() {
        start(&inner, call, cancellation).await;
        match preflight(&inner, assistant, call, &context, cancellation).await {
            Ok(input) => prepared.push((index, input)),
            Err(reason) => {
                let outcome = error(reason);
                end(&inner, call, &outcome, cancellation).await;
                outcomes[index] = Some(outcome);
            }
        }
    }
    let mut tasks = JoinSet::new();
    for (index, input) in prepared {
        let inner = inner.clone();
        let call = calls[index].clone();
        let assistant = assistant.clone();
        let context = context.clone();
        let cancellation = cancellation.clone();
        tasks.spawn(async move {
            let outcome =
                execute(&inner, &assistant, &call, &context, &cancellation, input).await?;
            end(&inner, &call, &outcome, &cancellation).await;
            Ok::<_, AgentError>((index, outcome))
        });
    }
    let mut failed = false;
    while let Some(joined) = tasks.join_next().await {
        match joined {
            Ok(Ok((index, outcome))) => outcomes[index] = Some(outcome),
            _ => failed = true,
        }
    }
    if failed {
        return Err(AgentError::RunFailed);
    }
    let terminating = !outcomes.is_empty()
        && outcomes
            .iter()
            .all(|o| o.as_ref().unwrap().0.terminate == Some(true));
    let mut messages = vec![];
    for (call, outcome) in calls.into_iter().zip(outcomes) {
        messages.push(record(&inner, call, outcome.unwrap(), cancellation, additions).await);
    }
    Ok((messages, terminating))
}
async fn start(inner: &Inner, call: &ToolCall, cancellation: &Cancellation) {
    emit(
        inner,
        AgentEvent::ToolExecutionStart {
            tool_call_id: call.id.clone(),
            tool_name: call.name.clone(),
            args: call.arguments().unwrap().clone(),
        },
        cancellation,
    )
    .await;
}
async fn end(inner: &Inner, call: &ToolCall, outcome: &Outcome, cancellation: &Cancellation) {
    emit(
        inner,
        AgentEvent::ToolExecutionEnd {
            tool_call_id: call.id.clone(),
            tool_name: call.name.clone(),
            result: outcome.0.clone(),
            is_error: outcome.1,
        },
        cancellation,
    )
    .await;
}
async fn record(
    inner: &Inner,
    call: ToolCall,
    (result, is_error): Outcome,
    cancellation: &Cancellation,
    additions: &mut Vec<AgentMessage>,
) -> ToolResultMessage {
    let timestamp = inner.options.clock.as_ref().map_or_else(
        || {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX)
        },
        |clock| clock(),
    );
    let message = ToolResultMessage {
        tool_call_id: call.id,
        tool_name: call.name,
        content: result.content,
        details: Some(result.details),
        is_error,
        timestamp,
    };
    let record = AgentMessage::Model(Message::ToolResult(message.clone()));
    emit(
        inner,
        AgentEvent::MessageStart {
            message: record.clone(),
        },
        cancellation,
    )
    .await;
    emit(
        inner,
        AgentEvent::MessageEnd {
            message: record.clone(),
        },
        cancellation,
    )
    .await;
    additions.push(record);
    message
}
async fn execute(
    inner: &Arc<Inner>,
    assistant: &AssistantMessage,
    call: &ToolCall,
    context: &AgentContext,
    cancellation: &Cancellation,
    (tool, args): Prepared,
) -> Result<Outcome, AgentError> {
    let (progress, delivery) =
        crate::progress::accept(inner.clone(), call.clone(), cancellation.clone());
    let invocation = crate::ToolInvocation {
        tool_call_id: call.id.clone(),
        args: args.clone(),
        cancellation: cancellation.clone(),
        progress,
    };
    let executed = tokio::spawn(async move { (tool.execute)(invocation).await }).await;
    // Even a failed executor must settle progress it accepted before unwinding.
    let delivered = delivery.await;
    let executed = executed.map_err(|_| AgentError::RunFailed)?;
    delivered.map_err(|_| AgentError::RunFailed)?;
    let (mut result, mut is_error) = match executed {
        Ok(result) => (result, false),
        Err(reason) => error(reason),
    };
    for hook in &inner.options.after_tool_call {
        match hook(
            crate::AfterToolCallContext {
                assistant_message: assistant.clone(),
                tool_call: call.clone(),
                args: args.clone(),
                result: result.clone(),
                is_error,
                context: context.clone(),
            },
            cancellation.clone(),
        )
        .await
        {
            Ok(replacement) => {
                if let Some(content) = replacement.content {
                    result.content = content;
                }
                if let Some(details) = replacement.details {
                    result.details = details;
                }
                if let Some(flag) = replacement.is_error {
                    is_error = flag;
                }
                if let Some(flag) = replacement.terminate {
                    result.terminate = Some(flag);
                }
            }
            Err(reason) => {
                (result, is_error) = error(reason);
                break;
            }
        }
    }
    Ok((result, is_error))
}
async fn preflight(
    inner: &Inner,
    assistant: &AssistantMessage,
    call: &ToolCall,
    context: &AgentContext,
    cancellation: &Cancellation,
) -> Result<Prepared, String> {
    let tool = inner
        .options
        .tools
        .iter()
        .find(|tool| tool.declaration.name == call.name)
        .ok_or_else(|| format!("Tool {} not found", call.name))?
        .clone();
    let mut args = call.arguments().unwrap().clone();
    if let Some(prepare) = &tool.prepare {
        args = prepare(args, cancellation.clone()).await?;
    }
    let prepared = ToolCall::new(
        call.id.clone(),
        call.name.clone(),
        args,
        call.replay_metadata.clone(),
    );
    let mut args = validate_tool_call(std::slice::from_ref(&tool.declaration), &prepared)
        .map_err(|error| error.to_string())?;
    for hook in &inner.options.before_tool_call {
        let outcome = hook(
            crate::BeforeToolCallContext {
                assistant_message: assistant.clone(),
                tool_call: call.clone(),
                args: args.clone(),
                context: context.clone(),
            },
            cancellation.clone(),
        )
        .await?;
        if outcome.block {
            return Err(outcome
                .reason
                .filter(|reason| !reason.is_empty())
                .unwrap_or_else(|| "Tool execution was blocked".into()));
        }
        if let Some(replacement) = outcome.args {
            args = replacement;
        }
    }
    Ok((tool, args))
}
fn error(reason: String) -> Outcome {
    (
        ToolResult {
            content: vec![InputContent::Text(TextContent {
                text: reason,
                replay_metadata: None,
            })],
            details: serde_json::json!({}),
            terminate: None,
        },
        true,
    )
}
