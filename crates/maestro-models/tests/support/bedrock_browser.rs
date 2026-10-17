//! Browser-only execution witness for the unsupported cloud conversation.

/// Browser-local callbacks and immediate terminal observations.
#[cfg(target_arch = "wasm32")]
mod browser {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use futures_util::FutureExt;
    use maestro_models::providers::cloud::amazon_bedrock::{BedrockOptions, stream_bedrock};
    use maestro_models::{AssistantMessageEvent, SharedHandle, StopReason, StreamOptions};
    use wasm_bindgen::{JsValue, prelude::wasm_bindgen};

    /// Independent counters retained by the witness and each supplied callback.
    #[derive(Default)]
    struct Counts {
        /// Request payload hook calls.
        payload: Arc<AtomicUsize>,
        /// Response hook calls.
        response: Arc<AtomicUsize>,
        /// Replacement sender calls.
        sender: Arc<AtomicUsize>,
    }

    impl Counts {
        /// Install local captures without borrowing the witness's state.
        fn options(&self) -> BedrockOptions {
            let payload = Arc::clone(&self.payload);
            let response = Arc::clone(&self.response);
            let sender = Arc::clone(&self.sender);
            BedrockOptions {
                common: StreamOptions {
                    on_payload: Some(SharedHandle::new(move |input, _| {
                        payload.fetch_add(1, Ordering::SeqCst);
                        Box::pin(std::future::ready(Ok(input)))
                    })),
                    on_response: Some(SharedHandle::new(move |_, _| {
                        response.fetch_add(1, Ordering::SeqCst);
                        Box::pin(std::future::ready(Ok(())))
                    })),
                    fetch: Some(Arc::new(move |_| {
                        sender.fetch_add(1, Ordering::SeqCst);
                        Box::pin(std::future::ready(Err(maestro_models::FetchError::Aborted)))
                    })),
                    ..StreamOptions::default()
                },
                ..BedrockOptions::default()
            }
        }
    }

    /// Require an observable fact without turning browser failures into anonymous traps.
    fn require(condition: bool, message: &str) -> Result<(), JsValue> {
        if condition {
            Ok(())
        } else {
            Err(JsValue::from_str(message))
        }
    }

    /// Execute the actual provider in a fresh browser, with immediately ready terminal futures.
    #[wasm_bindgen]
    pub fn browser_bedrock_call_ends_before_callbacks() -> Result<(), JsValue> {
        let corpus: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/cloud_runtime_witnesses.json"))
                .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let model = serde_json::from_value(corpus["common"]["model"].clone())
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let context = serde_json::from_value(corpus["common"]["context"].clone())
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let counts = Counts::default();
        let stream = stream_bedrock(model, context, Some(counts.options()));
        check_terminal(&stream)?;
        require(
            counts.payload.load(Ordering::SeqCst) == 0,
            "browser payload hook must remain uncalled",
        )?;
        require(
            counts.response.load(Ordering::SeqCst) == 0,
            "browser response hook must remain uncalled",
        )?;
        require(
            counts.sender.load(Ordering::SeqCst) == 0,
            "browser sender must remain uncalled",
        )
    }

    /// Require the error event, final diagnostic and stream end without awaiting pending work.
    fn check_terminal(stream: &maestro_models::AssistantMessageEventStream) -> Result<(), JsValue> {
        let event = stream
            .next()
            .now_or_never()
            .ok_or("browser error event must be ready")?;
        require(
            matches!(
                event,
                Some(AssistantMessageEvent::Error {
                    reason: maestro_models::ErrorReason::Error,
                    ..
                })
            ),
            "browser call must return one error event",
        )?;
        let result = stream
            .result()
            .now_or_never()
            .ok_or("browser result must be ready")?;
        let message = result
            .read()
            .map_err(|_| JsValue::from_str("result lock"))?;
        require(
            message.stop_reason == StopReason::Error,
            "browser stop reason must be error",
        )?;
        require(
            message.error_message.as_deref() == Some("Bedrock is not supported in browsers."),
            "browser diagnostic must match",
        )?;
        require(
            matches!(stream.next().now_or_never(), Some(None)),
            "browser stream must be ended",
        )?;
        Ok(())
    }
}
