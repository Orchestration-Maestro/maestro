//! Raw `xhigh` effort reaches the wire unchanged and the server decides.
#![allow(
    dead_code,
    reason = "each test binary uses a subset of the shared support"
)]

#[path = "support/bundled.rs"]
mod bundled;
#[path = "support/chat.rs"]
mod chat;

use chat::{TestResult, block_on};
use maestro_models::{
    FetchError, HttpResponse, Model, ProviderStreamOptions, StopReason, StreamOptions, get_model,
    reset_api_providers, stream,
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex, PoisonError};

/// `xhigh` as raw effort.
fn raw_xhigh(fetch: maestro_models::Fetch) -> TestResult<ProviderStreamOptions> {
    Ok(ProviderStreamOptions {
        common: StreamOptions {
            api_key: Some("controlled-key".into()),
            fetch: Some(fetch),
            ..StreamOptions::default()
        },
        extra: serde_json::from_value(json!({"reasoningEffort": "xhigh"}))?,
        ..ProviderStreamOptions::default()
    })
}

/// A transport answering every request with `status` and `body`, recording the bodies sent.
fn answering(status: u16, body: Vec<u8>) -> (maestro_models::Fetch, Arc<Mutex<Vec<Value>>>) {
    let bodies = Arc::new(Mutex::new(Vec::new()));
    let recorder = Arc::clone(&bodies);
    let fetch: maestro_models::Fetch = Arc::new(move |request| {
        recorder
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(serde_json::from_slice(&request.body).unwrap_or(Value::Null));
        let chunk: maestro_models::HttpBody =
            Box::pin(futures_util::stream::iter([Ok(body.clone())]));
        Box::pin(std::future::ready(Ok::<_, FetchError>(HttpResponse {
            status,
            status_text: String::new(),
            headers: [("content-type".to_owned(), "text/event-stream".to_owned())].into(),
            body: chunk,
        })))
    });
    (fetch, bodies)
}

/// Controlled reasoning summary then an answer, in the response wire format.
fn thinking_body() -> Vec<u8> {
    bundled::sse_frames(&[
        json!({"type":"response.output_item.added","item":{"type":"reasoning","id":"rs","summary":[]}}),
        json!({"type":"response.reasoning_summary_part.added","part":{"type":"summary_text","text":""}}),
        json!({"type":"response.reasoning_summary_text.delta","delta":"weighing"}),
        json!({"type":"response.reasoning_summary_part.done"}),
        json!({"type":"response.output_item.done","item":{"type":"reasoning","id":"rs","summary":[{"type":"summary_text","text":"weighing"}]}}),
        json!({"type":"response.output_item.added","item":{"type":"message","id":"m","content":[]}}),
        json!({"type":"response.content_part.added","part":{"type":"output_text","text":""}}),
        json!({"type":"response.output_text.delta","delta":"answer"}),
        json!({"type":"response.output_item.done","item":{"type":"message","id":"m","content":[{"type":"output_text","text":"answer"}]}}),
        json!({"type":"response.completed","response":{"id":"r","status":"completed","usage":{"input_tokens":3,"output_tokens":2,"total_tokens":5}}}),
    ])
}

/// A catalog model.
fn catalog(provider: &str, id: &str) -> TestResult<Model> {
    get_model(provider, id).ok_or_else(|| format!("{provider}/{id} is in the catalog").into())
}

#[test]
fn maestro_raw_xhigh_returns_thinking() -> TestResult {
    let _registry = bundled::registry();
    reset_api_providers();
    let (fetch, bodies) = answering(200, thinking_body());
    let model = catalog("openai", "gpt-5.1-codex-max")?;
    let result = block_on(false, async {
        Ok(
            stream(model, bundled::conversation()?, Some(raw_xhigh(fetch)?))?
                .result()
                .await,
        )
    })?;
    assert_eq!(bundled::stop_of(&result), StopReason::Stop);
    assert_eq!(bundled::text_of(&result), "answer");
    let thinking = result.read().map_err(|_| "poisoned")?.content.iter().any(|block| {
        matches!(block, maestro_models::AssistantContent::Thinking(thought) if thought.thinking == "weighing")
    });
    assert!(thinking, "the reasoning summary is kept as thinking");
    assert_eq!(
        bodies.lock().map_err(|_| "poisoned")?[0]["reasoning"]["effort"],
        "xhigh"
    );
    Ok(())
}

#[test]
fn maestro_raw_xhigh_preserves_server_rejection() -> TestResult {
    let _registry = bundled::registry();
    reset_api_providers();
    let rejection =
        br#"{"error":{"message":"Unsupported value: 'xhigh' is not supported with this model."}}"#
            .to_vec();
    let responses = catalog("openai", "gpt-5-mini")?;
    let mut chat_description = serde_json::to_value(&responses)?;
    chat_description["api"] = json!("openai-completions");
    if let Some(object) = chat_description.as_object_mut() {
        object.remove("compat");
    }
    let chat_model: Model = serde_json::from_value(chat_description)?;
    for (model, field) in [
        (responses, "/reasoning/effort"),
        (chat_model, "/reasoning_effort"),
    ] {
        let api = model.api.clone();
        let (fetch, bodies) = answering(400, rejection.clone());
        let result = block_on(false, async {
            Ok(
                stream(model, bundled::conversation()?, Some(raw_xhigh(fetch)?))?
                    .result()
                    .await,
            )
        })?;
        assert_eq!(bundled::stop_of(&result), StopReason::Error, "{api}");
        assert!(
            bundled::error_of(&result).is_some_and(|text| text.contains("xhigh")),
            "{api}"
        );
        let bodies = bodies.lock().map_err(|_| "poisoned")?;
        assert_eq!(
            bodies.first().and_then(|body| body.pointer(field)),
            Some(&json!("xhigh")),
            "{api}"
        );
    }
    Ok(())
}
