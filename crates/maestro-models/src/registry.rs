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
    overrides: HashMap<String, crate::CatalogOverride>,
    clock: Arc<dyn Fn() -> u64 + Send + Sync>,
}

impl Models {
    /// Construct an empty registry with a supplied Unix-millisecond clock.
    pub fn new(clock: Arc<dyn Fn() -> u64 + Send + Sync>) -> Self {
        Self {
            registrations: HashMap::new(),
            overrides: HashMap::new(),
            clock,
        }
    }

    /// Bind one explicit model identity to an adapter, rejecting duplicates.
    /// Empty required data or invalid chat declarations return InvalidCatalog.
    /// Programmatic registration relies on accounting's report-time rate checks;
    /// catalog and override data instead validate rates on ingestion.
    pub fn register(&mut self, model: Model, provider: Arc<dyn Provider>) -> Result<(), Failure> {
        crate::catalog::validate_structure(&model)?;
        if self.registrations.contains_key(&model.identity) {
            return Err(Failure::DuplicateModel);
        }
        self.registrations
            .insert(model.identity.clone(), (model, provider));
        Ok(())
    }

    /// Materialize a trusted local nonblocking getter once, replacing this provider.
    /// Configuration loading belongs to the caller; lookups never rerun the getter.
    /// Catalog and override data validate rates on ingestion; programmatic
    /// registration relies on accounting's report-time rate checks.
    /// Entire batches validate before publication: duplicate identities return
    /// DuplicateModel; provider mismatch or invalid metadata returns InvalidCatalog.
    /// Getter errors return fixed CatalogFailed diagnostics. All failures preserve
    /// usable registrations, adapters and overrides. Empty valid catalogs remove
    /// this provider's base entries while keeping overrides for future matches.
    pub fn register_catalog<F>(
        &mut self,
        provider: &str,
        getter: F,
        adapter: Arc<dyn Provider>,
    ) -> Result<(), Failure>
    where
        F: FnOnce() -> Result<Vec<Model>, Failure>,
    {
        let entries = getter().map_err(|_| Failure::CatalogFailed)?;
        crate::catalog::validate_batch(provider, &entries)?;
        self.registrations.retain(|id, _| id.provider != provider);
        for model in entries {
            self.registrations
                .insert(model.identity.clone(), (model, adapter.clone()));
        }
        Ok(())
    }

    /// Return independent local metadata without auth or I/O.
    /// None includes all operations; registry order is not selection policy.
    pub fn known(&self, operation: Option<&str>) -> Vec<Model> {
        self.registrations
            .values()
            .filter(|(model, _)| operation.is_none_or(|op| model.identity.operation == op))
            .map(|(model, _)| self.effective(model))
            .collect()
    }

    /// Look up an owned model by its complete opaque identity.
    pub fn find(&self, identity: &ModelIdentity) -> Option<Model> {
        self.registrations
            .get(identity)
            .map(|(model, _)| self.effective(model))
    }

    fn effective(&self, base: &Model) -> Model {
        let mut model = base.clone();
        if let Some(overrides) = self.overrides.get(&base.identity.provider) {
            if let Some(endpoint) = &overrides.endpoint {
                model.endpoint = endpoint.clone();
            }
            if let Some(replacement) = overrides
                .models
                .iter()
                .find(|m| m.identity == base.identity)
            {
                model = replacement.clone();
            }
        }
        model
    }

    /// Supply reversible provider metadata without modifying base registrations.
    /// Requires a currently registered provider, otherwise UnknownProvider.
    /// Validate the entire override, including unmatched targets, before publication;
    /// InvalidCatalog or DuplicateModel preserves the previous override and base.
    /// Provider endpoint applies first, then complete model replacements win.
    /// Unmatched identities remain stored but never create registered entries.
    pub fn set_override(
        &mut self,
        provider: &str,
        catalog_override: crate::CatalogOverride,
    ) -> Result<(), Failure> {
        if !self.registrations.keys().any(|id| id.provider == provider) {
            return Err(Failure::UnknownProvider);
        }
        crate::catalog::validate_batch(provider, &catalog_override.models)?;
        if catalog_override
            .endpoint
            .as_ref()
            .is_some_and(String::is_empty)
        {
            return Err(Failure::InvalidCatalog);
        }
        self.overrides.insert(provider.into(), catalog_override);
        Ok(())
    }

    /// Remove supplied overrides, exposing the newest base registrations.
    /// Returns whether an override existed.
    pub fn remove_override(&mut self, provider: &str) -> bool {
        self.overrides.remove(provider).is_some()
    }

    /// Remove all provider registrations and overrides without changing captured requests.
    /// Returns whether any base or override state existed.
    pub fn remove_provider(&mut self, provider: &str) -> bool {
        let before = self.registrations.len();
        self.registrations.retain(|id, _| id.provider != provider);
        let removed_override = self.overrides.remove(provider).is_some();
        before != self.registrations.len() || removed_override
    }

    /// Filter owned entries using configured status and a metadata-only account predicate.
    /// Status is read once per represented provider; credentials are never resolved.
    /// The trusted predicate must not read secrets or block. Availability does not
    /// predict request success and order is not a selection policy.
    pub fn available(
        &self,
        operation: Option<&str>,
        resolver: &dyn crate::AuthResolver,
        account_filter: &dyn Fn(&Model) -> bool,
    ) -> Vec<crate::AvailableModel> {
        let mut statuses = HashMap::new();
        self.known(operation)
            .into_iter()
            .filter_map(|model| {
                let status = statuses
                    .entry(model.identity.provider.clone())
                    .or_insert_with(|| resolver.status(&model.identity.provider));
                (status.configured && account_filter(&model)).then(|| crate::AvailableModel {
                    model,
                    auth_status: status.clone(),
                })
            })
            .collect()
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
    ) -> Result<(Model, Arc<dyn Provider>), Failure> {
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
        let registered = self.effective(&registration.0);
        if registered.protocol != model.protocol {
            return Err(Failure::UnsupportedOperation);
        }
        let supported = registration.1.supports("chat");
        if options.cancellation.is_cancelled() {
            return Err(Failure::Cancelled);
        }
        if !supported {
            return Err(Failure::UnsupportedOperation);
        }
        Ok((registered, registration.1.clone()))
    }

    /// Resolve once and dispatch once with captured registered metadata.
    /// Projects once before dispatch without changing caller history.
    /// Samples the clock once; normalization retains the same cancellation signal.
    /// Resolution failures become terminal errors without invoking an adapter.
    /// Captures registered rates once; invocation metadata never overrides catalog prices.
    pub fn stream(&self, model: Model, context: Context, options: StreamOptions) -> ModelStream {
        let timestamp = (self.clock)();
        let rates = self
            .registrations
            .get(&model.identity)
            .and_then(|(registered, _)| self.effective(registered).rates.clone());
        let source = self
            .registration(&model, &options)
            .and_then(|(registered, provider)| {
                let effective = crate::options::resolve(&registered.capabilities, options.clone())?;
                if options.cancellation.is_cancelled() {
                    Err(Failure::Cancelled)
                } else {
                    let context = crate::project_context(
                        &context,
                        &registered,
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
        ModelStream::new(model, timestamp, source, options.cancellation, rates)
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
