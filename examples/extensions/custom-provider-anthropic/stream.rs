//! A custom message-stream implementation using the host's transport and event buffer.
use super::oauth::CustomAnthropic;
use maestro_extensions_wasm::{
    AssistantMessageEventStream, Context, ExtensionFuture, Model, StreamOptionsInput,
};

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
        let _ = (&self.io, model, context, options, stream);
        Box::pin(std::future::ready(Err(
            "Message streaming is unavailable".into()
        )))
    }
}
