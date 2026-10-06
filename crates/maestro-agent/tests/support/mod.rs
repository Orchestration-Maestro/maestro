#![allow(dead_code)]
use maestro_agent::*;
use maestro_models::*;
use std::sync::{Arc, Mutex};

pub fn user(text: &str) -> AgentMessage {
    AgentMessage::Model(Message::User(UserMessage {
        content: UserContent::Blocks(vec![InputContent::Text(TextContent {
            text: text.into(),
            text_signature: None,
        })]),
        timestamp: 17.0,
    }))
}
pub fn options() -> SimpleStreamOptions {
    SimpleStreamOptions {
        base: StreamOptions {
            signal: Some(Cancellation::new()),
            ..Default::default()
        },
        ..Default::default()
    }
}
pub fn steps(text: &str) -> Vec<Action> {
    vec![
        Action::Update(Update::TextStart { content_index: 0 }),
        Action::Update(Update::TextDelta {
            content_index: 0,
            delta: text.into(),
        }),
        Action::Update(Update::TextEnd {
            content_index: 0,
            text_signature: None,
        }),
        Action::Update(Update::Done {
            reason: StopReason::Stop,
        }),
    ]
}
pub fn setup(responses: Vec<Response>, config: AgentOptions) -> (Agent, Arc<ControlledAdapter>) {
    let model = Model {
        id: "text".into(),
        name: "Synthetic text".into(),
        api: "synthetic".into(),
        provider: "script".into(),
        base_url: "synthetic:endpoint".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec!["text".into()],
        cost: TokenRates {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 0.0,
        max_tokens: 0.0,
        headers: None,
        compat: None,
    };
    let adapter = Arc::new(ControlledAdapter {
        responses: Mutex::new(responses.into()),
        calls: Mutex::new(vec![]),
    });
    let supplied = adapter.clone();
    let provider = ApiProvider {
        api: model.api.clone(),
        stream: Arc::new(|_, _, _| panic!("raw callback is not used by agent")),
        stream_simple: Arc::new(move |model, context, options| {
            let call = Call {
                context,
                options: options.unwrap().base,
            };
            supplied.calls.lock().unwrap().push(call.clone());
            let response = supplied.responses.lock().unwrap().pop_front().unwrap();
            let stream = create_assistant_message_event_stream();
            let output = stream.clone();
            tokio::spawn(async move {
                let signal = call.options.signal.clone().unwrap();
                let actions = match response {
                    Response::Steps(actions) => actions,
                    Response::Factory(factory) => factory(call).await.unwrap(),
                    Response::Failure(failure) => {
                        output
                            .push(error_event(assistant(&model), failure))
                            .unwrap();
                        return;
                    }
                };
                let mut message = assistant(&model);
                output
                    .push(AssistantMessageEvent::Start {
                        partial: shared(&message),
                    })
                    .unwrap();
                for action in actions {
                    let update = match action {
                        Action::Wait(wait) => {
                            tokio::select! { biased;
                                _ = signal.cancelled() => {
                                    output.push(error_event(message, Failure::Cancelled)).unwrap();
                                    return;
                                }
                                _ = wait => {}
                            }
                            continue;
                        }
                        Action::Update(update) => update,
                    };
                    match update {
                        Update::TextStart { content_index } => {
                            message.content.push(AssistantContent::Text(TextContent {
                                text: String::new(),
                                text_signature: None,
                            }));
                            output
                                .push(AssistantMessageEvent::TextStart {
                                    content_index: content_index as f64,
                                    partial: shared(&message),
                                })
                                .unwrap();
                        }
                        Update::TextDelta {
                            content_index,
                            delta,
                        } => {
                            let AssistantContent::Text(text) = &mut message.content[content_index]
                            else {
                                panic!()
                            };
                            text.text.push_str(&delta);
                            output
                                .push(AssistantMessageEvent::TextDelta {
                                    content_index: content_index as f64,
                                    delta,
                                    partial: shared(&message),
                                })
                                .unwrap();
                        }
                        Update::TextEnd {
                            content_index,
                            text_signature,
                        } => {
                            let AssistantContent::Text(text) = &mut message.content[content_index]
                            else {
                                panic!()
                            };
                            text.text_signature = text_signature;
                            let content = text.text.clone();
                            output
                                .push(AssistantMessageEvent::TextEnd {
                                    content_index: content_index as f64,
                                    content,
                                    partial: shared(&message),
                                })
                                .unwrap();
                        }
                        Update::ThinkingStart {
                            content_index,
                            signature,
                        } => {
                            message
                                .content
                                .push(AssistantContent::Thinking(ThinkingContent {
                                    thinking: String::new(),
                                    thinking_signature: signature,
                                    redacted: None,
                                }));
                            output
                                .push(AssistantMessageEvent::ThinkingStart {
                                    content_index: content_index as f64,
                                    partial: shared(&message),
                                })
                                .unwrap();
                        }
                        Update::ThinkingDelta {
                            content_index,
                            delta,
                        } => {
                            let AssistantContent::Thinking(text) =
                                &mut message.content[content_index]
                            else {
                                panic!()
                            };
                            text.thinking.push_str(&delta);
                            output
                                .push(AssistantMessageEvent::ThinkingDelta {
                                    content_index: content_index as f64,
                                    delta,
                                    partial: shared(&message),
                                })
                                .unwrap();
                        }
                        Update::ThinkingEnd { content_index } => {
                            let AssistantContent::Thinking(text) = &message.content[content_index]
                            else {
                                panic!()
                            };
                            output
                                .push(AssistantMessageEvent::ThinkingEnd {
                                    content_index: content_index as f64,
                                    content: text.thinking.clone(),
                                    partial: shared(&message),
                                })
                                .unwrap();
                        }
                        Update::Usage { usage } => message.usage = usage,
                        Update::ResponseIdentity {
                            response_model,
                            response_id,
                        } => {
                            message.response_model = response_model;
                            message.response_id = response_id;
                        }
                        Update::Done { reason } => {
                            message.stop_reason = reason.clone();
                            output
                                .push(AssistantMessageEvent::Done {
                                    reason,
                                    message: shared(&message),
                                })
                                .unwrap();
                        }
                        Update::Error { failure } => {
                            output.push(error_event(message, failure)).unwrap();
                            return;
                        }
                    }
                }
            });
            Ok(stream)
        }),
    };
    (Agent::new(provider.stream_simple, model, config), adapter)
}
fn assistant(model: &Model) -> AssistantMessage {
    AssistantMessage {
        content: vec![],
        api: model.api.clone(),
        provider: model.provider.clone(),
        model: model.id.clone(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: zero_usage(),
        stop_reason: StopReason::Stop,
        error_message: None,
        timestamp: 42.0,
    }
}
fn shared(message: &AssistantMessage) -> Arc<std::sync::RwLock<AssistantMessage>> {
    Arc::new(std::sync::RwLock::new(message.clone()))
}
fn error_event(mut message: AssistantMessage, failure: Failure) -> AssistantMessageEvent {
    let reason = if failure == Failure::Cancelled {
        StopReason::Aborted
    } else {
        StopReason::Error
    };
    message.stop_reason = reason.clone();
    message.error_message = Some(failure.to_string());
    AssistantMessageEvent::Error {
        reason,
        error: shared(&message),
    }
}
#[derive(Clone)]
pub struct Call {
    pub context: Context,
    pub options: StreamOptions,
}
type ResponseFactory = Box<
    dyn FnOnce(
            Call,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<Vec<Action>, Failure>> + Send>,
        > + Send,
>;
pub enum Response {
    Steps(Vec<Action>),
    Factory(ResponseFactory),
    Failure(Failure),
}
pub enum Action {
    Update(Update),
    Wait(std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>),
}
pub enum Update {
    TextStart {
        content_index: usize,
    },
    TextDelta {
        content_index: usize,
        delta: String,
    },
    TextEnd {
        content_index: usize,
        text_signature: Option<String>,
    },
    ThinkingStart {
        content_index: usize,
        signature: Option<String>,
    },
    ThinkingDelta {
        content_index: usize,
        delta: String,
    },
    ThinkingEnd {
        content_index: usize,
    },
    Usage {
        usage: Usage,
    },
    ResponseIdentity {
        response_model: Option<String>,
        response_id: Option<String>,
    },
    Done {
        reason: StopReason,
    },
    Error {
        failure: Failure,
    },
}
pub struct ControlledAdapter {
    responses: Mutex<std::collections::VecDeque<Response>>,
    calls: Mutex<Vec<Call>>,
}
impl ControlledAdapter {
    pub fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }
}
pub fn capture(agent: &Agent) -> Arc<Mutex<Vec<AgentEvent>>> {
    let events = Arc::new(Mutex::new(Vec::new()));
    let output = events.clone();
    agent.subscribe(Arc::new(move |event, _| {
        output.lock().unwrap().push(event);
        Box::pin(async {})
    }));
    events
}
pub fn label(event: &AgentEvent) -> &'static str {
    match event {
        AgentEvent::AgentStart => "agent_start",
        AgentEvent::AgentEnd { .. } => "agent_end",
        AgentEvent::TurnStart => "turn_start",
        AgentEvent::TurnEnd { .. } => "turn_end",
        AgentEvent::MessageStart { .. } => "message_start",
        AgentEvent::MessageEnd { .. } => "message_end",
        AgentEvent::MessageUpdate { .. } => "message_update",
    }
}

pub fn gate() -> (
    Action,
    tokio::sync::oneshot::Receiver<()>,
    tokio::sync::oneshot::Sender<()>,
) {
    let (entered_tx, entered) = tokio::sync::oneshot::channel();
    let (release, release_rx) = tokio::sync::oneshot::channel();
    (
        Action::Wait(Box::pin(async move {
            entered_tx.send(()).unwrap();
            release_rx.await.unwrap();
        })),
        entered,
        release,
    )
}
pub async fn pending<F: std::future::Future + Unpin>(future: &mut F) {
    std::future::poll_fn(|cx| {
        assert!(std::pin::Pin::new(&mut *future).poll(cx).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
}

pub fn zero_usage() -> Usage {
    Usage {
        input: 0.0,
        output: 0.0,
        cache_read: 0.0,
        cache_write: 0.0,
        total_tokens: 0.0,
        cost: UsageCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total: 0.0,
        },
    }
}
