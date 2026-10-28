//! Opt-in registration of a custom message-stream provider.
#![cfg(target_arch = "wasm32")]
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionFuture, ModelCost, ModelInput, ProviderCallbacks, ProviderConfig,
    ProviderModelConfig, StreamFunction, StreamOptionsInput, create_assistant_message_event_stream,
};
use std::{
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

/// Subscription authorization through the host's retained HTTP interface.
mod oauth;
/// Request conversion and ordered streamed results.
mod stream;

/// Loads the opt-in provider through the component's public extension interface.
#[must_use]
pub fn custom_message_provider_extension(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let provider = Rc::new(oauth::CustomAnthropic {
            io: api.native_io(),
            api: api.clone(),
            now: Rc::new(now_millis),
        });
        api.register_provider(
            "custom-anthropic",
            ProviderConfig {
                base_url: Some("https://api.anthropic.com".into()),
                api_key: Some("CUSTOM_ANTHROPIC_API_KEY".into()),
                api: Some("custom-anthropic-api".into()),
                models: Some(models()),
                callbacks: ProviderCallbacks {
                    stream_simple: Some(stream_function(Rc::clone(&provider))),
                    oauth: Some(provider),
                },
                ..ProviderConfig::default()
            },
        )
    })
}

/// Runs each producer on the component's existing local executor.
fn stream_function(provider: Rc<oauth::CustomAnthropic>) -> StreamFunction<StreamOptionsInput> {
    Rc::new(move |model, context, options| {
        let stream = create_assistant_message_event_stream()?;
        let producing = stream.clone();
        let provider = Rc::clone(&provider);
        wit_bindgen::spawn_local(async move {
            if let Err(error) = provider
                .stream_custom_anthropic(model, context, options, producing)
                .await
            {
                eprintln!("Custom message provider stream failed: {error}");
            }
        });
        Ok(stream)
    })
}

/// The two authored model descriptors, with prices per million tokens.
fn models() -> Vec<ProviderModelConfig> {
    [
        (
            "claude-opus-4-5",
            "Claude Opus 4.5 (Custom)",
            ModelCost {
                input: 5.0,
                output: 25.0,
                cache_read: 0.5,
                cache_write: 6.25,
            },
        ),
        (
            "claude-sonnet-4-5",
            "Claude Sonnet 4.5 (Custom)",
            ModelCost {
                input: 3.0,
                output: 15.0,
                cache_read: 0.3,
                cache_write: 3.75,
            },
        ),
    ]
    .into_iter()
    .map(|(id, name, cost)| ProviderModelConfig {
        id: id.into(),
        name: name.into(),
        api: None,
        base_url: None,
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost,
        context_window: 200_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    })
    .collect()
}

/// Reads the component's wall clock in Unix epoch milliseconds.
fn now_millis() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |time| time.as_secs_f64() * 1000.0)
}

maestro_extensions_wasm::export_extension!(custom_message_provider_extension);
