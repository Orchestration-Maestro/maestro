//! Byte corpus exercised through the direct conversation operation.
#![cfg(test)]
use crate::{chat, chat::TestResult};
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
/// Prove dictionary key order recursively in retained tool arguments.
fn key_order(actual: &Value, expected: &Value, name: &str) {
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
                Arc::ptr_eq(crate::lifecycle::handle(&event), &result),
                "{name}: cumulative identity"
            );
            let mut value = serde_json::to_value(event)?;
            let object = value.as_object_mut().unwrap();
            for key in ["partial", "message", "error"] {
                object.remove(key);
            }
            events.push(value);
        }
        let mut value = serde_json::to_value(&*result.read().unwrap())?;
        value.as_object_mut().unwrap().remove("timestamp");
        if row["expected"].get("native_error").is_some() {
            assert_eq!(row["expected"]["native_error"], true, "{name}");
            assert!(
                value["errorMessage"]
                    .as_str()
                    .is_some_and(|message| !message.is_empty()),
                "{name}: expected native failure, got {value}"
            );
            value.as_object_mut().unwrap().remove("errorMessage");
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
