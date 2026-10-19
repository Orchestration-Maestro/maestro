//! Controlled wire bodies, models and request capture for the bundled protocols.

use crate::chat::{TestResult, context, model};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use maestro_models::{
    AssistantMessageEvent, AssistantMessageEventStream, BoxFuture, Context, Fetch, HttpBody,
    HttpRequest, HttpResponse, Model, SharedAssistantMessage, StopReason,
};
use serde_json::{Value, json};
use std::fmt::Write as _;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// One bundled protocol with the model that routes to it.
pub struct Protocol {
    /// Protocol identifier.
    pub api: &'static str,
    /// Final URL path segment of its request.
    pub path: &'static str,
    /// Model base URL.
    pub base_url: &'static str,
}

/// The six SSE fixture protocols in registration order.
pub const PROTOCOLS: [Protocol; 6] = [
    Protocol {
        api: "anthropic-messages",
        path: "/v1/messages",
        base_url: "https://controlled.invalid",
    },
    Protocol {
        api: "openai-completions",
        path: "/v1/chat/completions",
        base_url: "https://controlled.invalid/v1",
    },
    Protocol {
        api: "mistral-conversations",
        path: "/v1/chat/completions",
        base_url: "https://controlled.invalid",
    },
    Protocol {
        api: "openai-responses",
        path: "/v1/responses",
        base_url: "https://controlled.invalid/v1",
    },
    Protocol {
        api: "azure-openai-responses",
        path: "/v1/responses",
        base_url: "https://controlled.openai.azure.com/openai/v1",
    },
    Protocol {
        api: "openai-codex-responses",
        path: "/codex/responses",
        base_url: "https://controlled.invalid",
    },
];

/// The SSE fixture identifiers in fixture order.
pub fn apis() -> Vec<&'static str> {
    PROTOCOLS.iter().map(|protocol| protocol.api).collect()
}

/// How one registered protocol treats the caller's cancellation signal and response hook.
pub struct ProtocolPolicy {
    /// Registered protocol identifier.
    pub api: &'static str,
    /// The request uses the signal carried by the payload the payload hook returns.
    pub selected_signal: bool,
    /// The response hook observes each HTTP response before its body is read.
    pub response_hook: bool,
}

/// Every bundled protocol in registration order, with its test policy.
pub const PROTOCOL_POLICIES: [ProtocolPolicy; 9] = [
    ProtocolPolicy {
        api: "anthropic-messages",
        selected_signal: false,
        response_hook: true,
    },
    ProtocolPolicy {
        api: "openai-completions",
        selected_signal: false,
        response_hook: true,
    },
    ProtocolPolicy {
        api: "mistral-conversations",
        selected_signal: false,
        response_hook: false,
    },
    ProtocolPolicy {
        api: "openai-responses",
        selected_signal: false,
        response_hook: true,
    },
    ProtocolPolicy {
        api: "azure-openai-responses",
        selected_signal: false,
        response_hook: true,
    },
    ProtocolPolicy {
        api: "openai-codex-responses",
        selected_signal: false,
        response_hook: true,
    },
    ProtocolPolicy {
        api: "google-generative-ai",
        selected_signal: true,
        response_hook: false,
    },
    ProtocolPolicy {
        api: "google-vertex",
        selected_signal: true,
        response_hook: false,
    },
    ProtocolPolicy {
        api: "bedrock-converse-stream",
        selected_signal: false,
        response_hook: true,
    },
];

/// The complete bundled registry inventory, separate from the SSE fixture protocols.
pub fn registered_apis() -> Vec<&'static str> {
    PROTOCOL_POLICIES.iter().map(|policy| policy.api).collect()
}

/// The test policy of `api`, looked up before any request starts.
///
/// # Panics
///
/// Panics with the protocol's name when the inventory has no entry for it.
#[cfg(test)]
pub fn policy(api: &str) -> &'static ProtocolPolicy {
    PROTOCOL_POLICIES
        .iter()
        .find(|policy| policy.api == api)
        .unwrap_or_else(|| panic!("no protocol test policy for {api}"))
}

/// Serializes tests that read or change process-wide provider and cleanup registries.
static REGISTRY: Mutex<()> = Mutex::new(());

/// Hold provider and cleanup registry admission for one test.
pub fn registry() -> MutexGuard<'static, ()> {
    REGISTRY.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A descriptor for `protocol` that no catalog contains.
pub fn descriptor(api: &str, base_url: &str) -> TestResult<Model> {
    model(&json!({"api": api, "baseUrl": base_url, "id": format!("{api}-model")}))
}

/// The descriptor of one bundled protocol.
pub fn protocol_model(protocol: &Protocol) -> TestResult<Model> {
    descriptor(protocol.api, protocol.base_url)
}

/// A one-message conversation.
pub fn conversation() -> TestResult<Context> {
    context(&json!({"messages": [{"role": "user", "content": "hi"}]}))
}

/// The SSE frames of a one-text answer in `api`'s wire format.
pub fn text_body(api: &str, text: &str) -> Vec<u8> {
    let frames: Vec<String> = match api {
        "anthropic-messages" => [
            json!({"type":"message_start","message":{"id":"m","usage":{"input_tokens":3,"output_tokens":0}}}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"text"}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":text}}),
            json!({"type":"content_block_stop","index":0}),
            json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":2}}),
            json!({"type":"message_stop"}),
        ]
        .iter()
        .map(|event| format!("event: {}\ndata: {event}\n\n", event["type"].as_str().unwrap_or_default()))
        .collect(),
        "openai-completions" | "mistral-conversations" => [
            json!({"id":"r","model":"server","choices":[{"index":0,"finish_reason":null,"delta":{"content":text}}]}),
            json!({"id":"r","model":"server","choices":[{"index":0,"finish_reason":"stop","delta":{}}],"usage":{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}}),
        ]
        .iter()
        .map(|event| format!("data: {event}\n\n"))
        .chain(["data: [DONE]\n\n".to_owned()])
        .collect(),
        _ => [
            json!({"type":"response.output_item.added","item":{"type":"message","id":"m","content":[]}}),
            json!({"type":"response.content_part.added","part":{"type":"output_text","text":""}}),
            json!({"type":"response.output_text.delta","delta":text}),
            json!({"type":"response.output_item.done","item":{"type":"message","id":"m","content":[{"type":"output_text","text":text}]}}),
            json!({"type":"response.completed","response":{"id":"r","status":"completed","usage":{"input_tokens":3,"output_tokens":2,"total_tokens":5,"input_tokens_details":{"cached_tokens":0},"output_tokens_details":{"reasoning_tokens":0}}}}),
        ]
        .iter()
        .map(|event| format!("data: {event}\n\n"))
        .collect(),
    };
    frames.concat().into_bytes()
}

/// Data-only SSE frames, one per event.
pub fn sse_frames(events: &[Value]) -> Vec<u8> {
    let mut frames = String::new();
    for event in events {
        write!(frames, "data: {event}\n\n").ok();
    }
    frames.into_bytes()
}

/// A request the controlled transport received.
#[derive(Clone)]
pub struct Seen {
    /// Final URL.
    pub url: String,
    /// Header names (lowercase) and values.
    pub headers: Vec<(String, String)>,
    /// JSON body.
    pub body: Value,
}

/// Requests received so far.
pub type Requests = Arc<Mutex<Vec<Seen>>>;

/// A transport that records each request and answers with `body`.
pub fn fetch(body: Vec<u8>) -> (Fetch, Requests) {
    fetch_chunks(vec![body])
}

/// A transport that records each request and answers with `chunks` as separate body reads.
pub fn fetch_chunks(chunks: Vec<Vec<u8>>) -> (Fetch, Requests) {
    let requests = Requests::default();
    let seen = Arc::clone(&requests);
    let fetch: Fetch = Arc::new(move |request: HttpRequest| {
        seen.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(Seen {
                url: request.url,
                headers: request.headers.into_iter().collect(),
                body: serde_json::from_slice(&request.body).unwrap_or(Value::Null),
            });
        let chunk: HttpBody = Box::pin(futures_util::stream::iter(
            chunks.clone().into_iter().map(Ok),
        ));
        let response: BoxFuture<Result<HttpResponse, maestro_models::FetchError>> =
            Box::pin(std::future::ready(Ok(HttpResponse {
                status: 200,
                status_text: String::new(),
                headers: [("content-type".to_owned(), "text/event-stream".to_owned())].into(),
                body: chunk,
            })));
        response
    });
    (fetch, requests)
}

/// Requests recorded so far.
pub fn recorded(requests: &Requests) -> Vec<Seen> {
    requests
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

/// Drain a stream, returning its events.
pub async fn drain(stream: &AssistantMessageEventStream) -> Vec<AssistantMessageEvent> {
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }
    events
}

/// The text blocks of a finished message joined together.
pub fn text_of(message: &SharedAssistantMessage) -> String {
    let message = message.read().unwrap_or_else(PoisonError::into_inner);
    message
        .content
        .iter()
        .filter_map(|block| match block {
            maestro_models::AssistantContent::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect()
}

/// Stop reason of a finished message.
pub fn stop_of(message: &SharedAssistantMessage) -> StopReason {
    message
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .stop_reason
        .clone()
}

/// Error text of a finished message.
pub fn error_of(message: &SharedAssistantMessage) -> Option<String> {
    message
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .error_message
        .clone()
}

/// A synthetic account token for response sessions, otherwise an opaque controlled key.
pub fn api_key(api: &str) -> String {
    if api == "openai-codex-responses" {
        let payload = URL_SAFE_NO_PAD
            .encode(r#"{"https://api.openai.com/auth":{"chatgpt_account_id":"controlled"}}"#);
        format!("a.{payload}.b")
    } else {
        "controlled-key".into()
    }
}
