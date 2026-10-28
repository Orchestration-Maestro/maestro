//! Requests for the gateway's models, delegated to the built-in Anthropic and OpenAI protocols.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::models::{self, GitLabModel};
use super::provider::GitLabDuo;
use maestro_extensions_wasm::{
    AssistantMessage, AssistantMessageEvent, AssistantMessageEventStream, Context, ErrorReason,
    ExtensionFuture, ExtensionResult, Model, SimpleStreamOptions, StopReason, StreamOptionsInput,
    Usage, stream_simple_via,
};
use std::sync::{Arc, RwLock};

impl GitLabDuo {
    /// Streams one request into `stream`; any failure becomes the stream's error message.
    ///
    /// # Errors
    /// Returns a failure of the host's event buffer.
    pub fn stream_gitlab_duo(
        &self,
        model: Model,
        context: Context,
        options: Option<StreamOptionsInput>,
        stream: AssistantMessageEventStream,
    ) -> ExtensionFuture<'static, ()> {
        let provider = self.clone();
        Box::pin(async move {
            if let Err(error) = provider.forward(&model, context, options, &stream).await {
                let failed = AssistantMessage {
                    content: Vec::new(),
                    api: model.api,
                    provider: model.provider,
                    model: model.id,
                    response_model: None,
                    response_id: None,
                    diagnostics: None,
                    usage: Usage::default(),
                    stop_reason: StopReason::Error,
                    error_message: Some(error),
                    timestamp: (provider.now)(),
                };
                stream.push(AssistantMessageEvent::Error {
                    reason: ErrorReason::Error,
                    error: Arc::new(RwLock::new(failed)),
                })?;
            }
            stream.end(None)
        })
    }

    /// Obtains direct access, starts the built-in protocol and relays its events in order.
    async fn forward(
        &self,
        model: &Model,
        context: Context,
        options: Option<StreamOptionsInput>,
        stream: &AssistantMessageEventStream,
    ) -> ExtensionResult<()> {
        let mut options = simple(options);
        let gitlab_access_token = options
            .common
            .api_key
            .take()
            .filter(|key| !key.is_empty())
            .ok_or("No GitLab access token. Run /login gitlab-duo or set GITLAB_TOKEN")?;
        let configured =
            models::find(&model.id).ok_or_else(|| format!("Unknown model: {}", model.id))?;
        let direct_access = self.get_direct_access_token(&gitlab_access_token).await?;
        let mut headers = direct_access.headers;
        headers.insert(
            "Authorization".into(),
            format!("Bearer {}", direct_access.token),
        );
        options.common.api_key = Some("gitlab-duo".into());
        options.common.headers = Some(headers);
        let inner = stream_simple_via(
            configured.backend.api(),
            routed(model, configured),
            context,
            Some(options),
        )?;
        while let Some(event) = inner.next().await? {
            stream.push((*event).clone())?;
        }
        Ok(())
    }
}

/// The model addressed to the gateway's proxy for its built-in protocol.
fn routed(model: &Model, configured: &GitLabModel) -> Model {
    Model {
        base_url: configured.backend.base_url(),
        ..model.clone()
    }
}

/// Reads raw options as simple ones: only `reasoning` and `thinkingBudgets` carry over.
fn simple(options: Option<StreamOptionsInput>) -> SimpleStreamOptions {
    match options {
        Some(StreamOptionsInput::Simple(options)) => options,
        Some(StreamOptionsInput::Raw(raw)) => SimpleStreamOptions {
            reasoning: open_field(&raw.extra, "reasoning"),
            thinking_budgets: open_field(&raw.extra, "thinkingBudgets"),
            common: raw.common,
            tool_choice: None,
        },
        None => SimpleStreamOptions::default(),
    }
}

/// An open provider field, when it holds a value of the expected shape.
fn open_field<T: serde::de::DeserializeOwned>(
    extra: &serde_json::Map<String, serde_json::Value>,
    name: &str,
) -> Option<T> {
    extra
        .get(name)
        .and_then(|value| serde_json::from_value(value.clone()).ok())
}
