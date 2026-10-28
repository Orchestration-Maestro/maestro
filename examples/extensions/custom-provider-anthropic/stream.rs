//! A custom message-stream implementation using the host's transport and event buffer.
use super::oauth::CustomAnthropic;
use maestro_extensions_wasm::{
    AbortSignal, AssistantMessageEventStream, Context, ExtensionFuture, ExtensionResult, Model,
    StreamOptionsInput,
};

/// Message and request conversion.
#[path = "request.rs"]
mod request;
/// Shared assistant-message reduction.
#[path = "response.rs"]
mod response;
/// Byte-oriented event framing.
#[path = "sse.rs"]
mod sse;

impl CustomAnthropic {
    /// Produces one request's updates and terminal result in the supplied public stream.
    ///
    /// # Errors
    /// Returns failures from the host's event buffer.
    pub fn stream_custom_anthropic(
        &self,
        model: Model,
        context: Context,
        options: Option<StreamOptionsInput>,
        stream: AssistantMessageEventStream,
    ) -> ExtensionFuture<'static, ()> {
        let provider = self.clone();
        Box::pin(async move {
            let mut state = response::ResponseState::new(
                &model,
                &context,
                options.as_ref(),
                stream,
                (provider.now)(),
            );
            let signal = options
                .as_ref()
                .and_then(|options| options.common().signal.as_ref());
            state.begin()?;
            let outcome = if signal.is_some_and(AbortSignal::aborted) {
                Err("Request was aborted".into())
            } else {
                produce(&provider, &model, &context, options.as_ref(), &mut state).await
            };
            state.finish(outcome, signal.is_some_and(AbortSignal::aborted))
        })
    }
}

/// Consumes one host-owned HTTP body into the supplied response state.
async fn produce(
    provider: &CustomAnthropic,
    model: &Model,
    context: &Context,
    options: Option<&StreamOptionsInput>,
    state: &mut response::ResponseState,
) -> ExtensionResult<()> {
    let response = provider
        .io
        .fetch(request::build(model, context, options)?)
        .await?;
    if !response.ok() {
        // The status stays in the message, so the session's retry classification sees it.
        return Err(super::oauth::failure(&*response).await);
    }
    let body = response
        .body()
        .ok_or("Provider stream ended without a response body")?;
    let mut decoder = sse::Decoder::new();
    while let Some(bytes) = body.read().await? {
        for event in decoder.feed(&bytes)? {
            state.apply(&event)?;
        }
    }
    body.close()?;
    if !state.stopped {
        return Err("Provider stream ended without message_stop".into());
    }
    Ok(())
}
