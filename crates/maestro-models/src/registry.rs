//! Instance-owned explicit identity dispatch.

use crate::{
    AssistantMessage, Context, EffectiveOptions, Failure, Model, ModelEvent, ModelIdentity,
    ModelStream, Provider, StreamOptions,
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

    /// Resolve choices against registered metadata without dispatch, scripts,
    /// clock samples or events. Caller capabilities cannot override registration.
    /// Preserves lookup/protocol/operation failures; cancellation is checked before
    /// lookup and after the adapter capability callback. Enabled token budgets
    /// require a positive ceiling or return UnsupportedOperation.
    pub fn resolve_options(
        &self,
        model: &Model,
        options: StreamOptions,
    ) -> Result<EffectiveOptions, Failure> {
        let (registered, _) = self.registration(model, &options)?;
        crate::options::resolve(&registered.capabilities, options)
    }

    fn registration(
        &self,
        model: &Model,
        options: &StreamOptions,
    ) -> Result<&(Model, Arc<dyn Provider>), Failure> {
        if options.cancellation.is_cancelled() {
            return Err(Failure::Cancelled);
        }
        if model.identity.operation != "chat" {
            return Err(Failure::UnsupportedOperation);
        }
        if !self
            .registrations
            .keys()
            .any(|id| id.provider == model.identity.provider)
        {
            return Err(Failure::UnknownProvider);
        }
        let registration = self
            .registrations
            .get(&model.identity)
            .ok_or(Failure::UnknownModel)?;
        if registration.0.protocol != model.protocol {
            return Err(Failure::UnsupportedOperation);
        }
        let supported = registration.1.supports("chat");
        if options.cancellation.is_cancelled() {
            return Err(Failure::Cancelled);
        }
        if !supported {
            return Err(Failure::UnsupportedOperation);
        }
        Ok(registration)
    }

    /// Resolve once and dispatch once with captured registered metadata.
    /// Projects once before dispatch without changing caller history.
    /// Samples the clock once; normalization retains the same cancellation signal.
    /// Resolution failures become terminal errors without invoking an adapter.
    pub fn stream(&self, model: Model, context: Context, options: StreamOptions) -> ModelStream {
        let timestamp = (self.clock)();
        let source = self
            .registration(&model, &options)
            .and_then(|(registered, provider)| {
                let effective = crate::options::resolve(&registered.capabilities, options.clone())?;
                if options.cancellation.is_cancelled() {
                    Err(Failure::Cancelled)
                } else {
                    let context = crate::project_context(
                        &context,
                        &model,
                        &|id, model, source| {
                            if options.cancellation.is_cancelled() {
                                id.to_owned()
                            } else {
                                provider.normalize_tool_call_id(id, model, source)
                            }
                        },
                        timestamp,
                    );
                    if options.cancellation.is_cancelled() {
                        return Err(Failure::Cancelled);
                    }
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
                    crate::dispatch::source(
                        provider.clone(),
                        registered.clone(),
                        context,
                        resolved,
                        effective,
                    )
                }
            });
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
