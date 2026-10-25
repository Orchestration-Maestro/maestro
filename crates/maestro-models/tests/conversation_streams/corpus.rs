//! Byte corpus exercised through the direct conversation operation.
#![cfg(test)]
use crate::{chat, chat::TestResult, root_routes::key_order};
use maestro_models::{
    AssistantMessageEventStream, Fetch, HttpResponse, MistralOptions, Model, StreamOptions,
    stream_mistral,
};
use serde_json::{Value, json};
use std::sync::Arc;

/// The requested identity and rates used by the response corpus.
pub fn model() -> TestResult<Model> {
    chat::model(
        &json!({"id":"controlled-model", "api":"mistral-conversations", "provider":"oracle-no-env", "baseUrl":"https://fixture.invalid/v1", "cost":{"input":2,"output":7,"cacheRead":11,"cacheWrite":13}}),
    )
}

/// A complete admitted response from supplied bytes.
pub fn fetch(chunks: Vec<Vec<u8>>) -> Fetch {
    Arc::new(move |_| {
        let body = futures_util::stream::iter(chunks.clone().into_iter().map(Ok));
        Box::pin(std::future::ready(Ok(HttpResponse {
            status: 200,
            status_text: String::new(),
            headers: [("content-type".into(), "text/event-stream".into())].into(),
            body: Box::pin(body),
        })))
    })
}

/// Invoke the raw operation with a controlled credential and transport.
pub fn invoke(fetch: Fetch) -> TestResult<AssistantMessageEventStream> {
    Ok(stream_mistral(
        model()?,
        chat::context(&json!({"messages": []}))?,
        Some(MistralOptions {
            common: StreamOptions {
                api_key: Some("fixture-key".into()),
                fetch: Some(fetch),
                ..Default::default()
            },
            ..Default::default()
        }),
    ))
}

/// Compare all fields and array order, treating JSON numbers as their admitted doubles.
fn compare(actual: &Value, expected: &Value, name: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(e)) => assert_eq!(a.as_f64(), e.as_f64(), "{name}"),
        (Value::Object(a), Value::Object(e)) => {
            assert_eq!(a.len(), e.len(), "{name}: fields");
            for (key, value) in e {
                compare(&a[key], value, name);
            }
        }
        (Value::Array(a), Value::Array(e)) => {
            assert_eq!(a.len(), e.len(), "{name}: length");
            for (a, e) in a.iter().zip(e) {
                compare(a, e, name);
            }
        }
        _ => assert_eq!(actual, expected, "{name}"),
    }
}

/// Run every fixture owned by the named behavior test.
pub async fn rows(test: &str) -> TestResult {
    for row in chat::rows(include_str!("../fixtures/conversation-streams.json"), test)? {
        let name = row["name"].as_str().unwrap();
        let chunks: Vec<Vec<u8>> = serde_json::from_value(row["input"]["chunks"].clone())?;
        let stream = invoke(fetch(chunks))?;
        let result = stream.result().await;
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            assert!(
                Arc::ptr_eq(crate::response_output::handle(&event), &result),
                "{name}: cumulative identity"
            );
            let mut value = serde_json::to_value(event)?;
            let object = value.as_object_mut().unwrap();
            for key in ["partial", "message", "error"] {
                object.shift_remove(key);
            }
            events.push(value);
        }
        let mut value = serde_json::to_value(&*result.read().unwrap())?;
        value.as_object_mut().unwrap().shift_remove("timestamp");
        if row["expected"].get("native_error").is_some() {
            assert_eq!(row["expected"]["native_error"], true, "{name}");
            assert!(
                value["errorMessage"]
                    .as_str()
                    .is_some_and(|message| !message.is_empty()),
                "{name}: expected native failure, got {value}"
            );
            value.as_object_mut().unwrap().shift_remove("errorMessage");
        }
        // Fixed record field order is not an identity; tool dictionaries are.
        compare(&value, &row["expected"]["result"], name);
        compare(&Value::Array(events), &row["expected"]["events"], name);
        for (actual, expected) in value["content"]
            .as_array()
            .unwrap()
            .iter()
            .zip(row["expected"]["result"]["content"].as_array().unwrap())
        {
            if expected["type"] == "toolCall" {
                key_order(&actual["arguments"], &expected["arguments"], name);
            }
        }
    }
    Ok(())
}

/// Observe each encoded fragment while the producer waits for the next body read.
pub async fn recorded_roots() -> TestResult {
    for row in crate::root_routes::recorded()?.reasoning {
        recorded_root(row).await?;
    }
    Ok(())
}

/// Drive one root through creation, gated deltas and completion.
async fn recorded_root(row: crate::root_routes::StreamedRoot) -> TestResult {
    let (mut feed, body) = crate::messages::feed();
    let stream = invoke(crate::root_routes::body_fetch(body))?;
    feed.read_pending().await?;
    let mut identity = None;
    let mut observations = read_fragments(&stream, &mut feed, &row.chunks, &mut identity).await?;
    drop(feed.chunks);
    feed.released.await?;
    observations.push(tool_observation(&stream, &mut identity).await?);
    let final_message = stream.result().await;
    assert!(Arc::ptr_eq(identity.as_ref().unwrap(), &final_message));
    assert!(matches!(
        stream.next().await,
        Some(maestro_models::AssistantMessageEvent::Done { .. })
    ));
    assert!(stream.next().await.is_none());
    assert_eq!(observations, row.expected.events);
    let final_value = serde_json::to_value(&*final_message.read().unwrap())?;
    assert_eq!(final_value["content"], row.expected.content);
    key_order(
        &final_value["content"],
        &row.expected.content,
        "root routes",
    );
    Ok(())
}

/// Finish each supplied fragment before admitting its successor.
async fn read_fragments(
    stream: &AssistantMessageEventStream,
    feed: &mut crate::messages::Feed,
    chunks: &[String],
    identity: &mut Option<maestro_models::SharedAssistantMessage>,
) -> TestResult<Vec<Value>> {
    let mut observations = Vec::new();
    for (index, chunk) in chunks.iter().enumerate() {
        feed.chunks.send(chunk.as_bytes().to_vec())?;
        feed.read_pending().await?;
        for _ in 0..=usize::from(index == 0) {
            observations.push(tool_observation(stream, identity).await?);
        }
    }
    Ok(observations)
}

/// Project the actual tool event and its live retained content at the gated phase.
async fn tool_observation(
    stream: &AssistantMessageEventStream,
    identity: &mut Option<maestro_models::SharedAssistantMessage>,
) -> TestResult<Value> {
    let event = loop {
        let event = stream.next().await.ok_or("missing tool event")?;
        if !matches!(event, maestro_models::AssistantMessageEvent::Start { .. }) {
            break event;
        }
    };
    crate::root_routes::observation(&event, identity)
}
