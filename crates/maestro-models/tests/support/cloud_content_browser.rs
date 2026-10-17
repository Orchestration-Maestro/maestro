//! Browser content-generation invocation through injected global Fetch.

/// Browser-local invocation and observations.
#[cfg(target_arch = "wasm32")]
mod browser {
    use maestro_models::{
        GoogleVertexOptions, SharedHandle, SimpleStreamOptions, StopReason, StreamOptions,
        stream_google_vertex, stream_simple_google_vertex,
    };
    use serde_json::json;
    use std::{cell::Cell, rc::Rc};
    use wasm_bindgen::{JsValue, prelude::wasm_bindgen};

    /// Convert a failed browser observation into a useful JavaScript rejection.
    fn require(value: bool, message: &str) -> Result<(), JsValue> {
        if value {
            Ok(())
        } else {
            Err(JsValue::from_str(message))
        }
    }

    /// One model and context used by the cloud browser request.
    fn input() -> Result<(maestro_models::Model, maestro_models::Context), JsValue> {
        let model = serde_json::from_value(
            json!({"id":"gemini-fixture","name":"fixture","api":"google-vertex","provider":"google-vertex","baseUrl":"https://browser.invalid/v1","reasoning":false,"input":["text"],"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":128,"maxTokens":64}),
        );
        let context = serde_json::from_value(
            json!({"messages":[{"role":"user","content":"browser","timestamp":1}]}),
        );
        model
            .and_then(|model| context.map(|context| (model, context)))
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    /// Execute raw/simple cloud keys and keyless diagnostics without native credentials.
    #[wasm_bindgen]
    pub async fn cloud_browser_key_entry_has_no_native_dependencies() -> Result<(), JsValue> {
        let hooks = Rc::new(Cell::new(0));
        let responses = Rc::new(Cell::new(0));
        for simple in [false, true] {
            let (model, context) = input()?;
            let payload_count = Rc::clone(&hooks);
            let response_count = Rc::clone(&responses);
            let common = StreamOptions::<maestro_models::Threaded> {
                api_key: Some("fixture-key".into()),
                on_payload: Some(SharedHandle::new(move |input, _| {
                    payload_count.set(payload_count.get() + 1);
                    Box::pin(std::future::ready(Ok(input)))
                })),
                on_response: Some(SharedHandle::new(move |_, _| {
                    response_count.set(response_count.get() + 1);
                    Box::pin(std::future::ready(Ok(())))
                })),
                ..StreamOptions::default()
            };
            let stream = if simple {
                stream_simple_google_vertex(
                    model,
                    context,
                    Some(SimpleStreamOptions {
                        common,
                        ..SimpleStreamOptions::default()
                    }),
                )
            } else {
                stream_google_vertex(
                    model,
                    context,
                    Some(GoogleVertexOptions {
                        common,
                        ..GoogleVertexOptions::default()
                    }),
                )
            };
            let result = stream.result().await;
            let result = result
                .read()
                .map_err(|_| JsValue::from_str("result lock"))?;
            require(
                result.stop_reason == StopReason::Stop,
                "browser request must succeed",
            )?;
            require(
                matches!(result.content.first(),Some(maestro_models::AssistantContent::Text(text)) if text.text=="browser result"),
                "browser text must reach result",
            )?;
        }
        require(hooks.get() == 2, "both payload hooks must run")?;
        require(responses.get() == 0, "response hooks must not run")?;
        keyless_calls().await
    }

    /// Scope diagnostics precede the unavailable native browser authentication route.
    async fn keyless_calls() -> Result<(), JsValue> {
        for scope in [true, false] {
            let (model, context) = input()?;
            let stream = stream_google_vertex(
                model,
                context,
                Some(GoogleVertexOptions {
                    project: scope.then(|| "browser-project".into()),
                    location: scope.then(|| "global".into()),
                    ..GoogleVertexOptions::default()
                }),
            );
            let result = stream.result().await;
            let result = result
                .read()
                .map_err(|_| JsValue::from_str("result lock"))?;
            require(
                result.stop_reason == StopReason::Error,
                "keyless browser call must fail",
            )?;
            require(
                result.error_message.as_deref()
                    == Some(if scope {
                        "An API Key must be set when running in a browser"
                    } else {
                        "Vertex AI requires a project ID. Set GOOGLE_CLOUD_PROJECT/GCLOUD_PROJECT or pass project in options."
                    }),
                "browser setup diagnostic must match",
            )?;
        }
        Ok(())
    }
}
