//! Registered protocol dispatch over isolated loopback endpoints.
#![cfg(not(target_arch = "wasm32"))]

use crate::{bedrock, bundled, chat::TestResult, loopback, transport};
use maestro_models::{ProviderStreamOptions, StreamOptions, stream};
use serde_json::{Value, json};

/// Wire response for one text answer in each protocol family.
fn answer(api: &str) -> TestResult<Vec<u8>> {
    Ok(match api {
        "bedrock-converse-stream" => bedrock::answer("answer")?.concat(),
        "google-generative-ai" | "google-vertex" => bundled::sse_frames(&[json!({
            "candidates": [{"content": {"role": "model", "parts": [{"text": "answer"}]}, "finishReason": "STOP"}],
            "usageMetadata": {"promptTokenCount": 3, "candidatesTokenCount": 2, "totalTokenCount": 5}
        })]),
        _ => bundled::text_body(api, "answer"),
    })
}

#[test]
fn built_in_protocols_dispatch_the_same_loopback_answers() -> TestResult {
    let _admission = bundled::registry();
    if !maestro_test_harness::selection().is_child() {
        let output = maestro_test_harness::TestProcess::current(
            "built_in_protocols_dispatch_the_same_loopback_answers",
        )?
        .nocapture()
        .env_clear()
        .case("protocols")
        .output()?;
        assert!(output.status.success(), "{output:?}");
        print!("{}", String::from_utf8(output.stdout)?);
        return Ok(());
    }
    crate::runtime::run(async {
        for api in bundled::registered_apis() {
            let content_type = if api == "bedrock-converse-stream" {
                "application/vnd.amazon.eventstream"
            } else {
                "text/event-stream"
            };
            let head = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: {content_type}\r\nconnection: close\r\n\r\n"
            );
            let peer = loopback::serve(&head, loopback::Step::writes(vec![answer(api)?]))?;
            let model = bundled::descriptor(api, &peer.url)?;
            let options = ProviderStreamOptions {
                common: StreamOptions {
                    api_key: Some(bundled::api_key(api)),
                    transport: Some(maestro_models::Transport::Sse),
                    max_retries: Some(0.0),
                    ..StreamOptions::default()
                },
                extra: [("bearerToken".into(), json!("fixture-token"))]
                    .into_iter()
                    .collect(),
                ..ProviderStreamOptions::default()
            };
            let output = stream(model, bundled::conversation()?, Some(options))?;
            let (labels, message) = transport::drain(&output).await?;
            assert_eq!(message.error_message, None, "{api}");
            assert_eq!(
                labels,
                ["start", "text_start", "text_delta", "text_end", "done"],
                "{api}"
            );
            assert_eq!(
                serde_json::to_value(&message)?["content"][0]["text"],
                "answer",
                "{api}"
            );
            let mut message = serde_json::to_value(message)?;
            message["timestamp"] = Value::from(0);
            println!(
                "protocol-proof {api} {}",
                json!({"events": labels, "message": message})
            );
            assert!(peer.finish()?.request_line.starts_with("POST "));
        }
        Ok(())
    })
}
