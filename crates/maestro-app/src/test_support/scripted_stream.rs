//! A model stream that replays scripted answers block by block and records its requests.
use maestro_agent::StreamFn;
use maestro_models::{
    AssistantContent, AssistantMessage, AssistantMessageEvent, AssistantMessageEventStream,
    Context, DiagnosticErrorInfo, DoneReason, ErrorReason, Local, Model, SharedAssistantMessage,
    StopReason, TextContent, ThinkingContent, ToolCall,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
    sync::{Arc, RwLock},
};

/// One answer of the scripted stream: optional thinking, text and tool calls, a failure
/// and input and output usage.
#[derive(Clone, Default)]
pub struct Scripted {
    /// Thinking content.
    pub thinking: Option<String>,
    /// Text content.
    pub text: Option<String>,
    /// Tool calls by name and arguments.
    pub tool_calls: Vec<(String, Value)>,
    /// The failure text; the answer then stops with an error.
    pub error: Option<String>,
    /// Input and output tokens; 100 and 50 when absent.
    pub usage: Option<(f64, f64)>,
    /// Whether the stream sends only the terminal event, so the response arrives whole.
    pub terminal_only: bool,
    /// Whether the answer names the model the request asked for, as a provider's answer does.
    pub names_requested_model: bool,
}

impl Scripted {
    /// A text answer.
    #[must_use]
    pub fn text(text: &str) -> Self {
        Self {
            text: Some(text.into()),
            ..Self::default()
        }
    }

    /// A failed answer reporting `error`.
    #[must_use]
    pub fn failure(error: &str) -> Self {
        Self {
            error: Some(error.into()),
            ..Self::default()
        }
    }

    /// A failed answer from the requested model reporting a context overflow.
    #[must_use]
    pub fn context_overflow() -> Self {
        Self {
            names_requested_model: true,
            ..Self::failure("prompt is too long: 213462 tokens > 200000 maximum")
        }
    }

    /// A call of the tool named `echo` with `text`.
    #[must_use]
    pub fn echo_call(text: &str) -> Self {
        Self {
            tool_calls: vec![("echo".into(), json!({"text": text}))],
            ..Self::default()
        }
    }
}

/// The requests the scripted stream received, in order.
#[derive(Clone, Default)]
pub struct ScriptedRequests(pub Rc<RefCell<Vec<Context>>>);

/// The stream answering request `n` with `responses[n % len]`, each block streamed with its
/// start, its deltas and its end; a stream given no answers fails every request.
pub(super) fn scripted_stream(
    responses: Vec<Scripted>,
    requests: ScriptedRequests,
) -> StreamFn<Local> {
    Rc::new(move |model, context, _, _| {
        let request = requests.0.borrow().len();
        requests.0.borrow_mut().push(context);
        Box::pin(std::future::ready(answered(&responses, request, model)))
    })
}

/// The stream of the answer to request `request` from `model`.
fn answered(
    responses: &[Scripted],
    request: usize,
    model: Model,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
    let response = request
        .checked_rem(responses.len())
        .and_then(|index| responses.get(index))
        .ok_or("the scripted stream was given no answers")?;
    let mut message = scripted_message(response).map_err(|error| error.to_string())?;
    if response.names_requested_model {
        message.provider = model.provider;
        message.model = model.id;
    }
    let stream = AssistantMessageEventStream::new();
    if !response.terminal_only {
        stream_blocks(&stream, &message);
    }
    stream_terminal(&stream, message);
    Ok(stream)
}

/// The answer `response` describes.
fn scripted_message(response: &Scripted) -> serde_json::Result<AssistantMessage> {
    /// The tool calls scripted so far, which number each call's identity.
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    let mut content = Vec::new();
    if let Some(thinking) = &response.thinking {
        content.push(AssistantContent::Thinking(ThinkingContent {
            thinking: thinking.clone(),
            thinking_signature: None,
            redacted: None,
        }));
    }
    if let Some(text) = &response.text {
        content.push(AssistantContent::Text(TextContent::new(text.clone())));
    }
    for (name, arguments) in &response.tool_calls {
        let call = CALLS.fetch_add(1, Ordering::Relaxed) + 1;
        content.push(AssistantContent::ToolCall(ToolCall {
            id: format!("faux_tc_{call}"),
            name: name.clone(),
            arguments: arguments.clone(),
            thought_signature: None,
        }));
    }
    if content.is_empty() && response.error.is_none() {
        content.push(AssistantContent::Text(TextContent::new(String::new())));
    }
    let stop = match (&response.error, response.tool_calls.is_empty()) {
        (Some(_), _) => "error",
        (None, false) => "toolUse",
        (None, true) => "stop",
    };
    let (input, output) = response.usage.unwrap_or((100.0, 50.0));
    serde_json::from_value(json!({
        "role": "assistant", "content": content, "api": "anthropic-messages",
        "provider": "faux", "model": "faux-1", "stopReason": stop,
        "errorMessage": response.error, "timestamp": maestro_models::timestamp_now(),
        "usage": {"input": input, "output": output, "cacheRead": 0, "cacheWrite": 0,
            "totalTokens": input + output,
            "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0}},
    }))
}

/// Pushes the start of `message`, then each block with its start, deltas of up to four
/// characters and its end.
fn stream_blocks(stream: &AssistantMessageEventStream, message: &AssistantMessage) {
    let mut partial = AssistantMessage {
        content: Vec::new(),
        ..message.clone()
    };
    let snapshot = |partial: &AssistantMessage| Arc::new(RwLock::new(partial.clone()));
    stream.push(AssistantMessageEvent::Start {
        partial: snapshot(&partial),
    });
    for (index, block) in message.content.iter().enumerate() {
        partial.content.push(block.clone());
        let text = match block {
            AssistantContent::Thinking(thinking) => thinking.thinking.clone(),
            AssistantContent::Text(text) => text.text.clone(),
            AssistantContent::ToolCall(call) => call.arguments.to_string(),
        };
        let characters: Vec<char> = text.chars().collect();
        let mut chunks: Vec<String> = characters
            .chunks(4)
            .map(|chunk| chunk.iter().collect())
            .collect();
        if chunks.is_empty() {
            chunks.push(String::new());
        }
        stream.push(block_event(block, index, None, &snapshot(&partial)));
        for chunk in chunks {
            stream.push(block_event(block, index, Some(chunk), &snapshot(&partial)));
        }
        stream.push(block_end(block, index, &snapshot(&partial)));
    }
}

/// Pushes the terminal event of `message`: its completion, or its failure.
fn stream_terminal(stream: &AssistantMessageEventStream, message: AssistantMessage) {
    let failed = matches!(message.stop_reason, StopReason::Error | StopReason::Aborted);
    let reason = message.stop_reason.clone();
    let message = Arc::new(RwLock::new(message));
    stream.push(if failed {
        let reason = if reason == StopReason::Aborted {
            ErrorReason::Aborted
        } else {
            ErrorReason::Error
        };
        AssistantMessageEvent::Error {
            reason,
            error: message,
        }
    } else {
        let reason = match reason {
            StopReason::ToolUse => DoneReason::ToolUse,
            StopReason::Length => DoneReason::Length,
            _ => DoneReason::Stop,
        };
        AssistantMessageEvent::Done { reason, message }
    });
}

/// The start of `block`, or one of its deltas.
fn block_event(
    block: &AssistantContent,
    content_index: usize,
    delta: Option<String>,
    partial: &SharedAssistantMessage,
) -> AssistantMessageEvent {
    let partial = Arc::clone(partial);
    match (block, delta) {
        (AssistantContent::Thinking(_), None) => AssistantMessageEvent::ThinkingStart {
            content_index,
            partial,
        },
        (AssistantContent::Thinking(_), Some(delta)) => AssistantMessageEvent::ThinkingDelta {
            content_index,
            delta,
            partial,
        },
        (AssistantContent::Text(_), None) => AssistantMessageEvent::TextStart {
            content_index,
            partial,
        },
        (AssistantContent::Text(_), Some(delta)) => AssistantMessageEvent::TextDelta {
            content_index,
            delta,
            partial,
        },
        (AssistantContent::ToolCall(_), None) => AssistantMessageEvent::ToolcallStart {
            content_index,
            partial,
        },
        (AssistantContent::ToolCall(_), Some(delta)) => AssistantMessageEvent::ToolcallDelta {
            content_index,
            delta,
            partial,
        },
    }
}

/// The end of `block`.
fn block_end(
    block: &AssistantContent,
    content_index: usize,
    partial: &SharedAssistantMessage,
) -> AssistantMessageEvent {
    let partial = Arc::clone(partial);
    match block {
        AssistantContent::Thinking(thinking) => AssistantMessageEvent::ThinkingEnd {
            content_index,
            content: thinking.thinking.clone(),
            partial,
        },
        AssistantContent::Text(text) => AssistantMessageEvent::TextEnd {
            content_index,
            content: text.text.clone(),
            partial,
        },
        AssistantContent::ToolCall(call) => AssistantMessageEvent::ToolcallEnd {
            content_index,
            tool_call: call.clone(),
            partial,
        },
    }
}
