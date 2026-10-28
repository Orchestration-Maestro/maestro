//! A provider for GitLab Duo's Claude and GPT models, with GitLab sign-in and requests delegated to the built-in protocols.
#![cfg(target_arch = "wasm32")]
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionFuture, ProviderCallbacks, ProviderConfig, StreamFunction,
    StreamOptionsInput, create_assistant_message_event_stream,
};
use std::{
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

/// The service addresses and the model table.
mod models;
/// GitLab sign-in and token refresh.
mod oauth;
/// The provider's retained effects and its direct access token.
mod provider;
/// Requests delegated to the built-in protocols.
mod stream;

/// Registers the opt-in provider through the component's public extension interface.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let provider = Rc::new(provider::GitLabDuo {
            io: api.native_io(),
            api: api.clone(),
            now: Rc::new(now_millis),
            random: Rc::new(random_bytes),
            direct_access: Rc::default(),
        });
        api.register_provider(
            "gitlab-duo",
            ProviderConfig {
                base_url: Some(models::AI_GATEWAY_URL.into()),
                api_key: Some("GITLAB_TOKEN".into()),
                api: Some("gitlab-duo-api".into()),
                models: Some(models::provider_models()),
                callbacks: ProviderCallbacks {
                    oauth: Some(provider.clone()),
                    stream_simple: Some(stream_function(provider)),
                },
                ..ProviderConfig::default()
            },
        )
    })
}

/// Runs each producer on the component's existing local executor.
fn stream_function(provider: Rc<provider::GitLabDuo>) -> StreamFunction<StreamOptionsInput> {
    Rc::new(move |model, context, options| {
        let stream = create_assistant_message_event_stream()?;
        let (producing, provider) = (stream.clone(), Rc::clone(&provider));
        wit_bindgen::spawn_local(async move {
            if let Err(error) = provider
                .stream_gitlab_duo(model, context, options, producing)
                .await
            {
                eprintln!("GitLab Duo stream failed: {error}");
            }
        });
        Ok(stream)
    })
}

/// Takes exactly 16 secure bytes from the existing WASI random interface.
fn random_bytes() -> Result<[u8; 16], String> {
    maestro_extensions_wasm::wasi::random::random::get_random_bytes(16)
        .try_into()
        .map_err(|_| "WASI random source did not return 16 bytes".into())
}

/// Reads the component's wall clock in Unix epoch milliseconds.
fn now_millis() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |time| time.as_secs_f64() * 1000.0)
}

maestro_extensions_wasm::export_extension!(factory);
