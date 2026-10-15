//! Executable, offline model-library examples.

mod accounts;
mod providers;
mod streams;
mod tools;

use crate::bundled;
use maestro_models::StreamOptions;

/// An explicit key and a finite, recorded chat response; no network is used.
fn response_options(api: &str, text: &str) -> (StreamOptions, bundled::Requests) {
    let (fetch, requests) = bundled::fetch(bundled::text_body(api, text));
    (
        StreamOptions {
            api_key: Some("controlled-key".into()),
            fetch: Some(fetch),
            ..Default::default()
        },
        requests,
    )
}

/// A tool declaration, not permission to execute model-supplied arguments.
fn weather_tool() -> maestro_models::Tool {
    maestro_models::Tool {
        name: "get_weather".into(),
        description: "Look up weather".into(),
        parameters: serde_json::json!({"type": "object", "properties": {
            "location": {"type": "string", "minLength": 1},
            "units": {"type": "string", "enum": ["celsius", "fahrenheit"], "default": "celsius"}
        }, "required": ["location"]}),
    }
}

/// Wire the existing gated body into the existing controlled HTTP transport.
fn fed_options() -> (crate::messages::Feed, StreamOptions) {
    let (feed, body) = crate::messages::feed();
    let body = std::sync::Mutex::new(Some(body));
    let transport =
        crate::transport::transport(vec![crate::transport::Attempt::streaming(move || {
            body.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
                .unwrap_or_else(|| {
                    Box::pin(futures_util::stream::iter([Err(
                        maestro_models::FetchError::Aborted,
                    )]))
                })
        })]);
    (
        feed,
        StreamOptions {
            api_key: Some("controlled-key".into()),
            fetch: Some(transport.fetch),
            ..Default::default()
        },
    )
}
