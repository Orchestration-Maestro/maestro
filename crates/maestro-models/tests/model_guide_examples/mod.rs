//! Executable, offline model-library examples.

mod accounts;
mod providers;
mod streams;
mod tools;

#[cfg(not(target_arch = "wasm32"))]
use crate::bundled;
#[cfg(not(target_arch = "wasm32"))]
use maestro_models::StreamOptions;

#[cfg(not(target_arch = "wasm32"))]
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
        }, "required": ["location"]})
        .into(),
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Wire the existing gated body into the existing controlled HTTP transport.
fn fed_options() -> (crate::messages::Feed, StreamOptions) {
    let (feed, body) = crate::messages::feed();
    let transport =
        crate::transport::transport(vec![crate::transport::Attempt::streaming_once(body)]);
    (
        feed,
        StreamOptions {
            api_key: Some("controlled-key".into()),
            fetch: Some(transport.fetch),
            ..Default::default()
        },
    )
}
