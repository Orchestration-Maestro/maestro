//! Consumers of the shared JSON boundary keep their caller-owned decisions.
#![allow(
    dead_code,
    reason = "each test binary uses a subset of the shared support"
)]

#[path = "support/bundled.rs"]
mod bundled;
#[path = "support/chat.rs"]
mod chat;
#[path = "support/child_process.rs"]
mod child_process;

use bundled::PROTOCOLS;
use chat::{TestResult, block_on};
use maestro_models::{MistralOptions, OnPayload, StopReason, StreamOptions};
use serde_json::{Value, json};
use std::sync::Arc;

/// Nested numeric keys and numbers that only the canonical writer spells this way.
fn discriminating() -> Value {
    json!({"z": 1, "10": [0.0, 9_007_199_254_740_993_u64], "2": {"b": -0.0, "1": 1e21}})
}

#[test]
fn compact_json_reexport_keeps_wire_bytes() -> TestResult {
    let expected = r#"{"2":{"1":1e+21,"b":0},"10":[0,9007199254740992],"z":1}"#;
    assert_eq!(maestro_models::compact_json(&discriminating())?, expected);
    assert_eq!(maestro_json::compact_json(&discriminating())?, expected);
    Ok(())
}

/// Run one Mistral request whose payload the hook replaces with `edit` of the built payload.
async fn run(
    edit: impl Fn(Value) -> Value + Send + Sync + 'static,
) -> TestResult<(String, Vec<bundled::Seen>)> {
    let protocol = &PROTOCOLS[2];
    let (fetch, requests) = bundled::fetch(bundled::text_body(protocol.api, "ok"));
    let on_payload: OnPayload = Arc::new(move |payload, _| {
        let edited = edit(payload);
        Box::pin(async move { Ok(edited) })
    });
    let common = StreamOptions {
        api_key: Some(bundled::api_key(protocol.api)),
        transport: Some(maestro_models::Transport::Sse),
        fetch: Some(fetch),
        on_payload: Some(on_payload),
        ..StreamOptions::default()
    };
    let stream = maestro_models::stream_mistral(
        bundled::protocol_model(protocol)?,
        bundled::conversation()?,
        Some(MistralOptions {
            common,
            ..MistralOptions::default()
        }),
    );
    let result = stream.result().await;
    let outcome = match bundled::stop_of(&result) {
        StopReason::Error => bundled::error_of(&result).unwrap_or_default(),
        _ => bundled::text_of(&result),
    };
    Ok((outcome, bundled::recorded(&requests)))
}

/// A built payload with `fields` set over it.
fn with(mut payload: Value, fields: Value) -> Value {
    if let (Some(payload), Value::Object(fields)) = (payload.as_object_mut(), fields) {
        payload.extend(fields);
    }
    payload
}

#[test]
fn mistral_checks_object_fields_after_payload_hook() -> TestResult {
    block_on(true, async {
        let (text, sent) = run(|payload| {
            with(
                payload,
                json!({"temperature": null, "parallelToolCalls": false, "prediction": {"type": "content", "content": "x"}}),
            )
        })
        .await?;
        assert_eq!(text, "ok");
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].body["temperature"], Value::Null);
        assert_eq!(sent[0].body["parallel_tool_calls"], json!(false));
        assert_eq!(
            sent[0].body["prediction"],
            json!({"type": "content", "content": "x"})
        );

        for rejected in [
            json!({"parallelToolCalls": null}),
            json!({"responseFormat": [{"type": "text"}]}),
        ] {
            let case = rejected.clone();
            let (text, sent) = run(move |payload| with(payload, case.clone())).await?;
            assert!(
                text.starts_with("Input validation failed: "),
                "{rejected}: {text}"
            );
            assert!(sent.is_empty(), "{rejected}");
        }

        let (text, sent) = run(|payload| {
            let messages = payload["messages"].clone();
            serde_json::from_str(&format!(
                r#"{{"model":"lost","model":"kept","messages":{messages}}}"#
            ))
            .unwrap_or(Value::Null)
        })
        .await?;
        assert_eq!(text, "ok");
        assert_eq!(sent[0].body["model"], json!("kept"));
        Ok(())
    })
}
