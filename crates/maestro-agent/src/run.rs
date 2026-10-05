//! One model path with authoritative terminal records and ordered events.
use crate::{
    AgentError, AgentEvent, AgentMessage, Queue, StopAfterTurnContext, agent::Inner, events::emit,
};
use maestro_models::{
    AssistantMessage, Cancellation, Context, Message, ModelEvent, StopReason, StreamOptions,
};
use std::sync::Arc;

pub(crate) async fn run(
    inner: Arc<Inner>,
    mut input: Vec<AgentMessage>,
    mut skip_initial: bool,
    options: StreamOptions,
) -> Result<Vec<AgentMessage>, AgentError> {
    let cancellation = &options.cancellation;
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
        let context = inner.lock().context.clone();
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
        let mut stream = inner.models.stream(
            inner.model.clone(),
            Context {
                system_prompt: context.system_prompt,
                messages,
                tools: vec![],
            },
            options.clone(),
        );
        let mut started = false;
        let terminal = loop {
            let event = stream
                .next()
                .await
                .expect("model stream supplies a terminal outcome");
            match event {
                ModelEvent::Start { partial } => {
                    started = true;
                    emit(
                        &inner,
                        AgentEvent::MessageStart {
                            message: AgentMessage::Model(Message::Assistant(partial)),
                        },
                        cancellation,
                    )
                    .await;
                }
                ModelEvent::Done { message, .. } | ModelEvent::Error { error: message, .. } => {
                    break message;
                }
                event => {
                    emit(
                        &inner,
                        AgentEvent::MessageUpdate {
                            message: partial(&event).clone(),
                            assistant_message_event: event,
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
            Some(StopReason::Error | StopReason::Aborted)
        ) {
            break;
        }
        if let Some(stop) = &inner.options.stop_after_turn {
            let context = inner.lock().context.clone();
            if stop(StopAfterTurnContext {
                message: terminal,
                tool_results: vec![],
                context,
                new_messages: additions.clone(),
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
fn partial(event: &ModelEvent) -> &AssistantMessage {
    match event {
        ModelEvent::Start { partial }
        | ModelEvent::TextStart { partial, .. }
        | ModelEvent::TextDelta { partial, .. }
        | ModelEvent::TextEnd { partial, .. }
        | ModelEvent::ThinkingStart { partial, .. }
        | ModelEvent::ThinkingDelta { partial, .. }
        | ModelEvent::ThinkingEnd { partial, .. }
        | ModelEvent::ToolCallStart { partial, .. }
        | ModelEvent::ToolCallDelta { partial, .. }
        | ModelEvent::ToolCallEnd { partial, .. } => partial,
        ModelEvent::Done { message, .. } => message,
        ModelEvent::Error { error, .. } => error,
    }
}
