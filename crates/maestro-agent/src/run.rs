//! One model path with authoritative terminal records and ordered events.
use crate::{
    AgentError, AgentEvent, AgentMessage, Queue, StopAfterTurnContext, agent::Inner, events::emit,
};
use maestro_models::{
    AssistantMessage, AssistantMessageEvent, Cancellation, Context, Message, SimpleStreamOptions,
    StopReason,
};
use std::sync::Arc;

pub(crate) async fn run(
    inner: Arc<Inner>,
    mut input: Vec<AgentMessage>,
    mut skip_initial: bool,
    options: SimpleStreamOptions,
) -> Result<Vec<AgentMessage>, AgentError> {
    let cancellation = options
        .base
        .signal
        .as_ref()
        .expect("admission supplies a signal");
    emit(&inner, AgentEvent::AgentStart, cancellation).await;
    let mut additions = vec![];
    loop {
        emit(&inner, AgentEvent::TurnStart, cancellation).await;
        append_input(&inner, input, &mut additions, cancellation).await;
        if !skip_initial {
            let steering = inner.lock().queues.drain(Queue::Steering);
            append_input(&inner, steering, &mut additions, cancellation).await;
            skip_initial = true;
        }
        let context = snapshot_context(&inner.lock().context);
        let records = if let Some(transform) = &inner.options.transform_context {
            transform(context.messages, cancellation.clone()).await
        } else {
            context.messages
        };
        let messages = if let Some(convert) = &inner.options.convert_messages {
            convert(records).await
        } else {
            records
                .into_iter()
                .filter_map(|record| match record {
                    AgentMessage::Model(message) => Some(message),
                    _ => None,
                })
                .collect()
        };
        let stream = (inner.stream_fn)(
            inner.model.clone(),
            Context {
                system_prompt: context.system_prompt,
                messages,
                tools: Some(vec![]),
            },
            Some(options.clone()),
        )
        .map_err(|_| AgentError::RunFailed)?;
        let mut iterator = stream.iter();
        let mut started = false;
        let terminal = loop {
            let event = iterator
                .next()
                .await
                .expect("model stream supplies a terminal outcome");
            match event {
                AssistantMessageEvent::Start { partial } => {
                    started = true;
                    emit(
                        &inner,
                        AgentEvent::MessageStart {
                            message: AgentMessage::Model(Message::Assistant(snapshot(&partial))),
                        },
                        cancellation,
                    )
                    .await;
                }
                AssistantMessageEvent::Done { message, .. }
                | AssistantMessageEvent::Error { error: message, .. } => {
                    break snapshot(&message);
                }
                event => {
                    emit(
                        &inner,
                        AgentEvent::MessageUpdate {
                            message: snapshot(partial(&event)),
                            assistant_message_event: detach(event),
                        },
                        cancellation,
                    )
                    .await;
                }
            }
        };
        let record = AgentMessage::Model(Message::Assistant(terminal.clone()));
        if !started {
            emit(
                &inner,
                AgentEvent::MessageStart {
                    message: record.clone(),
                },
                cancellation,
            )
            .await;
        }
        emit(
            &inner,
            AgentEvent::MessageEnd {
                message: record.clone(),
            },
            cancellation,
        )
        .await;
        additions.push(record);
        emit(
            &inner,
            AgentEvent::TurnEnd {
                message: terminal.clone(),
                tool_results: vec![],
            },
            cancellation,
        )
        .await;
        if matches!(
            terminal.stop_reason,
            StopReason::Error | StopReason::Aborted
        ) {
            break;
        }
        if let Some(stop) = &inner.options.stop_after_turn {
            let context = snapshot_context(&inner.lock().context);
            if stop(StopAfterTurnContext {
                message: terminal,
                tool_results: vec![],
                context,
                new_messages: additions.iter().map(snapshot_record).collect(),
            })
            .await
            {
                break;
            }
        }
        input = inner.lock().queues.drain(Queue::Steering);
        if input.is_empty() {
            input = inner.lock().queues.drain(Queue::FollowUp);
        }
        if input.is_empty() {
            break;
        }
    }
    emit(
        &inner,
        AgentEvent::AgentEnd {
            messages: additions.clone(),
        },
        cancellation,
    )
    .await;
    Ok(additions)
}
async fn append_input(
    inner: &Arc<Inner>,
    input: Vec<AgentMessage>,
    additions: &mut Vec<AgentMessage>,
    cancellation: &Cancellation,
) {
    for message in input {
        emit(
            inner,
            AgentEvent::MessageStart {
                message: message.clone(),
            },
            cancellation,
        )
        .await;
        emit(
            inner,
            AgentEvent::MessageEnd {
                message: message.clone(),
            },
            cancellation,
        )
        .await;
        additions.push(message);
    }
}
fn partial(event: &AssistantMessageEvent) -> &std::sync::Arc<std::sync::RwLock<AssistantMessage>> {
    match event {
        AssistantMessageEvent::Start { partial }
        | AssistantMessageEvent::TextStart { partial, .. }
        | AssistantMessageEvent::TextDelta { partial, .. }
        | AssistantMessageEvent::TextEnd { partial, .. }
        | AssistantMessageEvent::ThinkingStart { partial, .. }
        | AssistantMessageEvent::ThinkingDelta { partial, .. }
        | AssistantMessageEvent::ThinkingEnd { partial, .. }
        | AssistantMessageEvent::ToolcallStart { partial, .. }
        | AssistantMessageEvent::ToolcallDelta { partial, .. }
        | AssistantMessageEvent::ToolcallEnd { partial, .. } => partial,
        AssistantMessageEvent::Done { message, .. } => message,
        AssistantMessageEvent::Error { error, .. } => error,
    }
}

fn snapshot(message: &std::sync::Arc<std::sync::RwLock<AssistantMessage>>) -> AssistantMessage {
    snapshot_assistant(&message.read().unwrap_or_else(|p| p.into_inner()))
}
pub(crate) fn snapshot_assistant(message: &AssistantMessage) -> AssistantMessage {
    let mut message = message.clone();
    for content in &mut message.content {
        if let maestro_models::AssistantContent::ToolCall(call) = content {
            let owned = call.read().unwrap_or_else(|p| p.into_inner()).clone();
            *call = Arc::new(std::sync::RwLock::new(owned));
        }
    }
    message
}
pub(crate) fn detach(mut event: AssistantMessageEvent) -> AssistantMessageEvent {
    let owned = Arc::new(std::sync::RwLock::new(snapshot(partial(&event))));
    match &mut event {
        AssistantMessageEvent::Start { partial }
        | AssistantMessageEvent::TextStart { partial, .. }
        | AssistantMessageEvent::TextDelta { partial, .. }
        | AssistantMessageEvent::TextEnd { partial, .. }
        | AssistantMessageEvent::ThinkingStart { partial, .. }
        | AssistantMessageEvent::ThinkingDelta { partial, .. }
        | AssistantMessageEvent::ThinkingEnd { partial, .. }
        | AssistantMessageEvent::ToolcallStart { partial, .. }
        | AssistantMessageEvent::ToolcallDelta { partial, .. }
        | AssistantMessageEvent::ToolcallEnd { partial, .. } => *partial = owned,
        AssistantMessageEvent::Done { message, .. } => *message = owned,
        AssistantMessageEvent::Error { error, .. } => *error = owned,
    }
    if let AssistantMessageEvent::ToolcallEnd { tool_call, .. } = &mut event {
        let owned = tool_call.read().unwrap_or_else(|p| p.into_inner()).clone();
        *tool_call = Arc::new(std::sync::RwLock::new(owned));
    }
    event
}

pub(crate) fn snapshot_record(record: &AgentMessage) -> AgentMessage {
    match record {
        AgentMessage::Model(Message::Assistant(message)) => {
            AgentMessage::Model(Message::Assistant(snapshot_assistant(message)))
        }
        _ => record.clone(),
    }
}
pub(crate) fn snapshot_context(context: &crate::AgentContext) -> crate::AgentContext {
    crate::AgentContext {
        system_prompt: context.system_prompt.clone(),
        messages: context.messages.iter().map(snapshot_record).collect(),
    }
}
pub(crate) fn snapshot_event(event: &AgentEvent) -> AgentEvent {
    match event {
        AgentEvent::AgentStart => AgentEvent::AgentStart,
        AgentEvent::TurnStart => AgentEvent::TurnStart,
        AgentEvent::AgentEnd { messages } => AgentEvent::AgentEnd {
            messages: messages.iter().map(snapshot_record).collect(),
        },
        AgentEvent::TurnEnd {
            message,
            tool_results,
        } => AgentEvent::TurnEnd {
            message: snapshot_assistant(message),
            tool_results: tool_results.clone(),
        },
        AgentEvent::MessageStart { message } => AgentEvent::MessageStart {
            message: snapshot_record(message),
        },
        AgentEvent::MessageEnd { message } => AgentEvent::MessageEnd {
            message: snapshot_record(message),
        },
        AgentEvent::MessageUpdate {
            message,
            assistant_message_event,
        } => AgentEvent::MessageUpdate {
            message: snapshot_assistant(message),
            assistant_message_event: detach(assistant_message_event.clone()),
        },
    }
}
