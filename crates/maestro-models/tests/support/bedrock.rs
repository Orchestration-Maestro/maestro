//! Controlled Bedrock conversations: event-stream frames, a scripted sender and request capture.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use maestro_models::{
    BoxFuture, Context, Fetch, FetchError, HttpBody, HttpRequest, HttpResponse, Model,
};
use serde_json::{Value, json};

use crate::chat::TestResult;

/// The model every controlled conversation asks.
pub fn model(base_url: &str) -> TestResult<Model> {
    Ok(serde_json::from_value(json!({
        "id": "meta.llama-controlled", "name": "Controlled text", "api": "bedrock-converse-stream",
        "provider": "amazon-bedrock", "baseUrl": base_url, "reasoning": false, "input": ["text"],
        "cost": {"input": 1, "output": 2, "cacheRead": 3, "cacheWrite": 4},
        "contextWindow": 8192, "maxTokens": 1024
    }))?)
}

/// A conversation holding one user message.
pub fn context(text: &str) -> TestResult<Context> {
    Ok(serde_json::from_value(json!({
        "messages": [{"role": "user", "content": text, "timestamp": 1}]
    }))?)
}

/// The CRC-32 the event-stream framing uses.
pub fn crc32(bytes: &[u8]) -> u32 {
    !bytes.iter().fold(!0_u32, |crc, byte| {
        (0..8).fold(crc ^ u32::from(*byte), |crc, _| {
            (crc >> 1) ^ (0xEDB8_8320 & 0_u32.wrapping_sub(crc & 1))
        })
    })
}

/// One event-stream frame with the given string headers and JSON payload.
fn encode(headers: &[(&str, &str)], payload: &Value) -> TestResult<Vec<u8>> {
    let mut block = Vec::new();
    for (name, value) in headers {
        block.push(u8::try_from(name.len())?);
        block.extend_from_slice(name.as_bytes());
        block.push(7);
        block.extend_from_slice(&u16::try_from(value.len())?.to_be_bytes());
        block.extend_from_slice(value.as_bytes());
    }
    let payload = serde_json::to_vec(payload)?;
    let total = u32::try_from(16 + block.len() + payload.len())?;
    let mut frame = Vec::new();
    frame.extend_from_slice(&total.to_be_bytes());
    frame.extend_from_slice(&u32::try_from(block.len())?.to_be_bytes());
    frame.extend_from_slice(&crc32(&frame).to_be_bytes());
    frame.extend_from_slice(&block);
    frame.extend_from_slice(&payload);
    frame.extend_from_slice(&crc32(&frame).to_be_bytes());
    Ok(frame)
}

/// A stream event frame.
pub fn event(kind: &str, payload: &Value) -> TestResult<Vec<u8>> {
    encode(
        &[
            (":message-type", "event"),
            (":event-type", kind),
            (":content-type", "application/json"),
        ],
        payload,
    )
}

/// A stream exception frame.
pub fn exception(kind: &str, message: &str) -> TestResult<Vec<u8>> {
    encode(
        &[
            (":message-type", "exception"),
            (":exception-type", kind),
            (":content-type", "application/json"),
        ],
        &json!({"message": message}),
    )
}

/// The frames of an assistant message that answers `text` in one block.
pub fn answer(text: &str) -> TestResult<Vec<Vec<u8>>> {
    Ok(vec![
        event("messageStart", &json!({"role": "assistant"}))?,
        event(
            "contentBlockDelta",
            &json!({"contentBlockIndex": 0, "delta": {"text": text}}),
        )?,
        event("contentBlockStop", &json!({"contentBlockIndex": 0}))?,
        event("messageStop", &json!({"stopReason": "end_turn"}))?,
    ])
}

/// A response with the given status, headers and body chunks.
pub fn response(status: u16, headers: &[(&str, &str)], chunks: Vec<Vec<u8>>) -> HttpResponse {
    let body: HttpBody = Box::pin(futures_util::stream::iter(chunks.into_iter().map(Ok)));
    HttpResponse {
        status,
        status_text: String::new(),
        headers: headers
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect::<BTreeMap<_, _>>(),
        body,
    }
}

/// A sender and the requests it received.
pub struct Scripted {
    /// Sender to install in the options.
    pub fetch: Fetch,
    /// Every request received, in order.
    pub requests: Arc<Mutex<Vec<HttpRequest>>>,
}

impl Scripted {
    /// The requests received so far.
    pub fn seen(&self) -> TestResult<Vec<HttpRequest>> {
        Ok(self
            .requests
            .lock()
            .map_err(|error| error.to_string())?
            .clone())
    }
}

/// A sender that records each request and answers it with `reply`.
pub fn scripted(
    reply: impl Fn(usize) -> BoxFuture<Result<HttpResponse, FetchError>> + Send + Sync + 'static,
) -> Scripted {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&requests);
    let fetch: Fetch = Arc::new(move |request| {
        let position = seen.lock().map_or(0, |mut seen| {
            seen.push(request);
            seen.len() - 1
        });
        reply(position)
    });
    Scripted { fetch, requests }
}

/// A sender that answers every request with a successful stream of `chunks`.
pub fn serve(chunks: Vec<Vec<u8>>) -> Scripted {
    scripted(move |_| {
        let headers = [("x-amzn-requestid", "fixture-id")];
        Box::pin(std::future::ready(Ok(response(
            200,
            &headers,
            chunks.clone(),
        ))))
    })
}

/// A body that records its first read and then yields `chunks`.
pub fn watched(polled: Arc<std::sync::atomic::AtomicBool>, chunks: Vec<Vec<u8>>) -> HttpBody {
    use futures_util::StreamExt;
    Box::pin(
        futures_util::stream::once(async move {
            polled.store(true, std::sync::atomic::Ordering::SeqCst);
            futures_util::stream::iter(chunks.into_iter().map(Ok))
        })
        .flatten(),
    )
}

/// Sets its flag when dropped.
pub struct DropFlag(pub Arc<std::sync::atomic::AtomicBool>);

impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

/// A body that yields `before`, then waits for `gate`, then yields `after` and reports on `end`
/// that a reader reached its end. The guard is dropped when the body is dropped or finished.
pub fn gated(
    before: Vec<Vec<u8>>,
    gate: tokio::sync::oneshot::Receiver<()>,
    after: Vec<Vec<u8>>,
    guard: DropFlag,
    end: tokio::sync::oneshot::Sender<()>,
) -> HttpBody {
    use futures_util::StreamExt;
    let wait = futures_util::stream::once(async move {
        let _guard = guard;
        gate.await.ok();
        futures_util::stream::iter(after.into_iter().map(Ok))
    })
    .flatten();
    let ended = futures_util::stream::once(async move {
        end.send(()).ok();
        futures_util::stream::empty()
    })
    .flatten();
    Box::pin(
        futures_util::stream::iter(before.into_iter().map(Ok))
            .chain(wait)
            .chain(ended),
    )
}
