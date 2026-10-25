//! Recorded whole-root protocol inputs and observations.
#![cfg(not(target_arch = "wasm32"))]
use crate::chat::TestResult;
use maestro_models::{
    AssistantMessageEvent as Event, AssistantMessageEventStream, Fetch, HttpBody, HttpResponse,
    SharedAssistantMessage,
};
use serde::Deserialize;
use serde_json::Value;
use std::sync::{Arc, Mutex};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Routes {
    pub reasoning: Vec<StreamedRoot>,
    pub history: Vec<History>,
    pub cloud: Vec<Cloud>,
    pub responses: Vec<StreamedRoot>,
    pub content: Vec<Content>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamedRoot {
    pub chunks: Vec<String>,
    pub expected: RootOutput,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootOutput {
    pub events: Vec<Value>,
    pub content: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct History {
    #[serde(default)]
    pub input: Value,
    pub input_json: Option<String>,
    pub expected: HistoryOutput,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryOutput {
    pub chat: Value,
    pub responses: Value,
    pub message: Value,
    pub content: Value,
    pub cloud: Value,
    pub reasoning: Value,
}

pub fn recorded() -> TestResult<Routes> {
    Ok(serde_json::from_str(include_str!(
        "../fixtures/tool_argument_root_routes.json"
    ))?)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cloud {
    pub frames: Vec<Frame>,
    pub expected: RootOutput,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub kind: String,
    pub body: Value,
}

/// One-use controlled body transported outside its collection lock.
pub fn body_fetch(body: HttpBody) -> Fetch {
    let slot = Arc::new(Mutex::new(Some(body)));
    Arc::new(move |_| {
        let body = slot.lock().ok().and_then(|mut slot| slot.take());
        Box::pin(async move {
            Ok(HttpResponse {
                status: 200,
                status_text: String::new(),
                headers: [("content-type".into(), "text/event-stream".into())].into(),
                body: body.ok_or(maestro_models::FetchError::Aborted)?,
            })
        })
    })
}

/// Project one ordered update with its live content and retained message identity.
pub fn observation(
    event: &Event,
    identity: &mut Option<SharedAssistantMessage>,
) -> TestResult<Value> {
    let partial = crate::response_output::handle(event);
    if let Some(first) = identity {
        assert!(Arc::ptr_eq(first, partial));
    } else {
        *identity = Some(Arc::clone(partial));
    }
    let content = partial.read().map_err(|e| e.to_string())?.content.clone();
    let content = serde_json::to_value(content)?;
    let mut value = serde_json::to_value(event)?;
    let object = value.as_object_mut().ok_or("event object")?;
    object.shift_remove("partial");
    object.insert("contentState".into(), content);
    Ok(value)
}

/// Freeze each frame at the next pending body read before observing its updates.
pub async fn gated(
    stream: &AssistantMessageEventStream,
    mut feed: crate::messages::Feed,
    mut chunks: Vec<Vec<u8>>,
    expected: RootOutput,
) -> TestResult {
    use futures_util::FutureExt as _;
    let mut events = Vec::new();
    let mut identity = None;
    let terminal_chunk = chunks.pop().ok_or("no terminal chunk")?;
    feed.read_pending().await?;
    for chunk in chunks {
        feed.chunks.send(chunk)?;
        feed.read_pending().await?;
        while let Some(Some(event)) = stream.next().now_or_never() {
            if !matches!(event, Event::Start { .. }) {
                events.push(observation(&event, &mut identity)?);
            }
        }
    }
    feed.chunks.send(terminal_chunk)?;
    drop(feed.chunks);
    feed.released.await?;
    let (terminal, result) = crate::response_output::collect(stream).await?;
    assert_eq!(terminal.len(), 1);
    assert_eq!(terminal[0]["type"], "done");
    assert_eq!(result["content"], expected.content);
    assert_eq!(events, expected.events);
    key_order(&result["content"], &expected.content, "root routes");
    assert!(Arc::ptr_eq(
        identity.as_ref().ok_or("no content update")?,
        &stream.result().await
    ));
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Content {
    pub route: ContentRoute,
    pub part: Value,
    pub expected: RootOutput,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentRoute {
    Direct,
    Cloud,
}

/// Prove dictionary key order recursively in retained tool arguments.
pub fn key_order(actual: &Value, expected: &Value, name: &str) {
    match (actual, expected) {
        (Value::Object(a), Value::Object(e)) => {
            assert_eq!(
                a.keys().collect::<Vec<_>>(),
                e.keys().collect::<Vec<_>>(),
                "{name}: keys"
            );
            for (key, value) in e {
                key_order(&a[key], value, name);
            }
        }
        (Value::Array(a), Value::Array(e)) => {
            for (a, e) in a.iter().zip(e) {
                key_order(a, e, name);
            }
        }
        _ => {}
    }
}
