//! Beta selection at public Messages invocation boundaries.

#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use crate::{chat::TestResult, messages};
use maestro_models::{SimpleStreamOptions, StreamOptions, stream_anthropic, stream_simple};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

/// Simple thinking-off requests never select the interleaved-thinking beta.
#[test]
fn messages_thinking_off_omits_interleaved_beta() -> TestResult {
    crate::runtime::run(async {
        let requests = messages::Requests::default();
        let stream = stream_simple(
            messages::model(&Value::Null)?,
            messages::context(&Value::Null)?,
            Some(SimpleStreamOptions {
                common: StreamOptions {
                    api_key: Some("fixture-key".into()),
                    fetch: Some(messages::scripted_fetch(
                        &messages::Case::default(),
                        &requests,
                    )),
                    ..StreamOptions::default()
                },
                ..SimpleStreamOptions::default()
            }),
        )?;
        messages::collect(&stream).await?;
        assert_eq!(
            requests.lock().unwrap()[0]["headers"]["anthropic-beta"],
            Value::Null
        );
        Ok(())
    })
}

/// Caller beta strings replace defaults, are deduplicated, and reach injected clients.
#[test]
fn messages_beta_replacement_reaches_injected_client() -> TestResult {
    crate::runtime::run(async {
        for (configured, expected) in [
            (None, json!(["interleaved-thinking-2025-05-14"])),
            (
                Some(" custom, custom , second, "),
                json!(["custom", "second"]),
            ),
            (Some(""), Value::Null),
            (Some(" , "), Value::Null),
        ] {
            let sent = Arc::new(Mutex::new(Value::Null));
            let kept = Arc::clone(&sent);
            let mut options = messages::options(&messages::Case::default())?;
            options.thinking_enabled = Some(true);
            options.common.headers =
                configured.map(|value| [("AnThRoPiC-BeTa".into(), value.into())].into());
            options.client = Some(Arc::new(move |payload, _| {
                *kept.lock().unwrap() = payload;
                Box::pin(std::future::ready(Ok(maestro_models::HttpResponse {
                    status: 200,
                    status_text: String::new(),
                    headers: std::collections::BTreeMap::default(),
                    body: Box::pin(futures_util::stream::iter([Ok(messages::STOP
                        .as_bytes()
                        .to_vec())])),
                })))
            }));
            let model = messages::model(&json!({"headers": {"anthropic-beta": "model-beta"}}))?;
            let model = if configured.is_none() {
                messages::model(&Value::Null)?
            } else {
                model
            };
            let stream = stream_anthropic(model, messages::context(&Value::Null)?, Some(options));
            messages::collect(&stream).await?;
            assert_eq!(sent.lock().unwrap()["betas"], expected);
        }
        Ok(())
    })
}

/// Payload hooks see the selected betas, while HTTP receives only the normalized header.
#[test]
fn messages_beta_payload_becomes_transport_header() -> TestResult {
    crate::runtime::run(async {
        for (configured, expected) in [
            (" custom, custom , second, ", "custom,second"),
            ("", ""),
            (" , ", " , "),
        ] {
            let requests = messages::Requests::default();
            let seen = Arc::new(Mutex::new(Value::Null));
            let kept = Arc::clone(&seen);
            let mut options = messages::options(&messages::Case::default())?;
            options.common.headers = Some([("Anthropic-Beta".into(), configured.into())].into());
            options.common.fetch = Some(messages::scripted_fetch(
                &messages::Case::default(),
                &requests,
            ));
            options.common.on_payload = Some(Arc::new(move |input, _| {
                *kept.lock().unwrap() = input.payload.clone();
                Box::pin(std::future::ready(Ok(input)))
            }));
            let stream = stream_anthropic(
                messages::model(&json!({"headers": {"anthropic-beta": "model-beta"}}))?,
                messages::context(&Value::Null)?,
                Some(options),
            );
            messages::collect(&stream).await?;
            let requests = requests.lock().unwrap();
            assert_eq!(requests[0]["headers"]["anthropic-beta"], expected);
            assert_eq!(requests[0]["body"]["betas"], Value::Null);
            let expected_payload = if configured == " custom, custom , second, " {
                json!(["custom", "second"])
            } else {
                Value::Null
            };
            assert_eq!(seen.lock().unwrap()["betas"], expected_payload);
        }
        Ok(())
    })
}
