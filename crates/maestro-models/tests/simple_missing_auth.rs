//! Missing authentication is reported before the simple entry returns a stream.

#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use crate::{chat::TestResult, messages};
use maestro_models::{SimpleStreamOptions, StreamOptions, stream_simple};
use serde_json::{Value, json};

/// A custom provider has no ambient key and must not invoke the transport.
fn rejects_missing_auth(api: &str) -> TestResult {
    let model = messages::model(&json!({"api": api, "provider": "missing-auth-fixture"}))?;
    let result = stream_simple(
        model,
        messages::context(&Value::Null)?,
        Some(SimpleStreamOptions {
            common: StreamOptions {
                fetch: Some(std::sync::Arc::new(|_| panic!("unexpected request"))),
                ..StreamOptions::default()
            },
            ..SimpleStreamOptions::default()
        }),
    );
    let Err(error) = result else {
        panic!("missing auth returned a stream")
    };
    assert_eq!(
        error.message,
        "No API key for provider: missing-auth-fixture"
    );
    Ok(())
}

/// Chat-completions rejects missing auth synchronously.
#[test]
fn chat_simple_missing_auth_returns_no_stream() -> TestResult {
    rejects_missing_auth("openai-completions")
}

/// Responses rejects missing auth synchronously.
#[test]
fn responses_simple_missing_auth_returns_no_stream() -> TestResult {
    rejects_missing_auth("openai-responses")
}

/// Azure Responses rejects missing auth synchronously.
#[test]
fn azure_simple_missing_auth_returns_no_stream() -> TestResult {
    rejects_missing_auth("azure-openai-responses")
}

/// Codex Responses rejects missing auth synchronously.
#[test]
fn codex_simple_missing_auth_returns_no_stream() -> TestResult {
    rejects_missing_auth("openai-codex-responses")
}

/// Messages rejects missing auth synchronously.
#[test]
fn messages_simple_missing_auth_returns_no_stream() -> TestResult {
    rejects_missing_auth("anthropic-messages")
}

/// Google content generation rejects missing auth synchronously.
#[test]
fn google_simple_missing_auth_returns_no_stream() -> TestResult {
    rejects_missing_auth("google-generative-ai")
}

/// Mistral rejects missing auth synchronously.
#[test]
fn mistral_simple_missing_auth_returns_no_stream() -> TestResult {
    rejects_missing_auth("mistral-conversations")
}
