//! Instance-owned explicit identity dispatch.

use crate::{
    AssistantMessage, Context, Failure, Model, ModelEvent, ModelIdentity, ModelStream, Provider,
    StreamOptions,
};
use std::collections::HashMap;
use std::sync::Arc;

/// Instance-local model registration and invocation.
pub struct Models {
    registrations: HashMap<ModelIdentity, (Model, Arc<dyn Provider>)>,
    clock: Arc<dyn Fn() -> u64 + Send + Sync>,
}

impl Models {
    /// Construct an empty registry with a supplied Unix-millisecond clock.
    pub fn new(clock: Arc<dyn Fn() -> u64 + Send + Sync>) -> Self {
        Self {
            registrations: HashMap::new(),
            clock,
        }
    }

    /// Bind one explicit model identity to an adapter, rejecting duplicates.
    pub fn register(&mut self, model: Model, provider: Arc<dyn Provider>) -> Result<(), Failure> {
        if self.registrations.contains_key(&model.identity) {
            return Err(Failure::DuplicateModel);
        }
        self.registrations
            .insert(model.identity.clone(), (model, provider));
        Ok(())
    }

    /// Read supplied configured-auth metadata for a registered provider only.
    /// This never resolves credentials or predicts request success.
    pub fn auth_status(
        &self,
        provider: &str,
        resolver: &dyn crate::AuthResolver,
    ) -> Result<crate::AuthStatus, Failure> {
        if !self
            .registrations
            .keys()
            .any(|identity| identity.provider == provider)
        {
            return Err(Failure::UnknownProvider);
        }
        Ok(resolver.status(provider))
    }

    /// Resolve once and return the normalized caller stream, including setup failures.
    /// Samples the clock once; the stream owns its source and requested metadata.
    pub fn stream(&self, model: Model, context: Context, options: StreamOptions) -> ModelStream {
        let timestamp = (self.clock)();
        let source = if options.cancellation.is_cancelled() {
            Err(Failure::Cancelled)
        } else if model.identity.operation != "chat" {
            Err(Failure::UnsupportedOperation)
        } else if !self
            .registrations
            .keys()
            .any(|identity| identity.provider == model.identity.provider)
        {
            Err(Failure::UnknownProvider)
        } else {
            self.registrations
                .get(&model.identity)
                .ok_or(Failure::UnknownModel)
                .and_then(|(registered, provider)| {
                    if registered.protocol != model.protocol || !provider.supports("chat") {
                        Err(Failure::UnsupportedOperation)
                    } else if options.cancellation.is_cancelled() {
                        Err(Failure::Cancelled)
                    } else {
                        let description = provider.description();
                        if options.cancellation.is_cancelled() {
                            return Err(Failure::Cancelled);
                        }
                        let mut resolved = options.clone();
                        resolved.headers = crate::dispatch::headers(&[
                            &description.headers,
                            &registered.headers,
                            &options.headers,
                        ])?;
                        crate::dispatch::source(provider.clone(), model.clone(), context, resolved)
                    }
                })
        };
        ModelStream::new(model, timestamp, source, options.cancellation)
    }

    /// Drain exactly one call to stream and return its terminal assistant record.
    /// Completion shares streaming dispatch, assembly and failure handling.
    pub async fn complete(
        &self,
        model: Model,
        context: Context,
        options: StreamOptions,
    ) -> AssistantMessage {
        let mut stream = self.stream(model, context, options);
        loop {
            match stream.next().await {
                Some(ModelEvent::Done { message, .. }) => return message,
                Some(ModelEvent::Error { error, .. }) => return error,
                _ => {}
            }
        }
    }
}
