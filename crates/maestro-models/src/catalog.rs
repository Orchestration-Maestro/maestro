//! Local inert catalog metadata.

use crate::{AuthStatus, Model, ModelIdentity};
use std::collections::BTreeMap;

/// Declared USD per million tokens; default zeros do not promise free access.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FlatRates {
    /// Input token rate.
    pub input: f64,
    /// Output token rate.
    pub output: f64,
    /// Cached input read rate.
    pub cache_read: f64,
    /// Cache write rate.
    pub cache_write: f64,
}

/// Declared chat capability metadata, not measured capacity or option resolution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatMetadata {
    /// Declared context limit; absent means unspecified.
    pub context_window: Option<u64>,
    /// Declared output limit; absent means unspecified.
    pub max_output_tokens: Option<u64>,
    /// Whether reasoning is declared.
    pub reasoning: bool,
    /// Open thinking names; explicit None disables a name, unlike an absent key.
    pub thinking_level_map: BTreeMap<String, Option<String>>,
}

/// Reversible provider endpoint and complete operation-qualified model replacements.
#[derive(Clone, Default, PartialEq)]
pub struct CatalogOverride {
    /// Optional provider endpoint, applied before full model replacements.
    pub endpoint: Option<String>,
    /// Complete replacements; unmatched identities wait for later registration.
    pub models: Vec<Model>,
}

/// Owned effective metadata with configured status, not a promise of live success.
#[derive(Clone, Debug, PartialEq)]
pub struct AvailableModel {
    /// Owned effective model.
    pub model: Model,
    /// Non-secret configured authentication metadata.
    pub auth_status: AuthStatus,
}

impl Model {
    /// Construct editable sparse metadata without validation.
    /// Chat limits are fallback declarations, not measured capacity; zero rates
    /// are unspecified pricing. Other operations receive no invented chat limits.
    pub fn custom(identity: ModelIdentity, protocol: String, endpoint: String) -> Self {
        let chat = (identity.operation == "chat").then(|| ChatMetadata {
            context_window: Some(128_000),
            max_output_tokens: Some(16_384),
            reasoning: false,
            thinking_level_map: BTreeMap::new(),
        });
        Self {
            name: identity.model.clone(),
            input: if chat.is_some() {
                vec!["text".into()]
            } else {
                vec![]
            },
            identity,
            protocol,
            endpoint,
            chat,
            headers: BTreeMap::new(),
            rates: FlatRates::default(),
            rates_supplied: false,
        }
    }
}

pub(crate) fn validate(model: &Model) -> Result<(), crate::Failure> {
    let required = [
        &model.identity.provider,
        &model.identity.model,
        &model.identity.operation,
        &model.protocol,
        &model.endpoint,
        &model.name,
    ];
    let rates = [
        model.rates.input,
        model.rates.output,
        model.rates.cache_read,
        model.rates.cache_write,
    ];
    let chat_valid = match (&model.chat, model.identity.operation.as_str()) {
        (Some(chat), "chat") => chat.context_window != Some(0) && chat.max_output_tokens != Some(0),
        (None, op) => op != "chat",
        _ => false,
    };
    if required.iter().any(|s| s.is_empty())
        || model.input.iter().any(String::is_empty)
        || rates.iter().any(|r| !r.is_finite() || *r < 0.0)
        || !chat_valid
    {
        return Err(crate::Failure::InvalidCatalog);
    }
    Ok(())
}

pub(crate) fn validate_batch(provider: &str, models: &[Model]) -> Result<(), crate::Failure> {
    if provider.is_empty() {
        return Err(crate::Failure::InvalidCatalog);
    }
    let mut identities = std::collections::HashSet::new();
    for model in models {
        validate(model)?;
        if model.identity.provider != provider {
            return Err(crate::Failure::InvalidCatalog);
        }
        if !identities.insert(&model.identity) {
            return Err(crate::Failure::DuplicateModel);
        }
    }
    Ok(())
}
