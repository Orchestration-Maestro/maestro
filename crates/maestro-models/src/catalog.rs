//! Local inert catalog metadata.

use crate::{AuthStatus, Model, ModelIdentity};
use std::collections::BTreeMap;

/// Declared chat capability metadata, not measured capacity or option resolution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatMetadata {
    /// Declared context limit; absent means unspecified.
    pub context_window: Option<u64>,
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
    /// Chat limits are fallback declarations, not measured capacity. Absent rates
    /// mean unknown pricing. Other operations receive no invented chat limits.
    pub fn custom(identity: ModelIdentity, protocol: String, endpoint: String) -> Self {
        let chat = (identity.operation == "chat").then_some(ChatMetadata {
            context_window: Some(128_000),
        });
        Self {
            name: identity.model.clone(),
            input: if chat.is_some() {
                vec!["text".into()]
            } else {
                vec![]
            },
            capabilities: crate::RequestCapabilities {
                output_limit: if chat.is_some() { 16_384 } else { 0 },
                ..Default::default()
            },
            identity,
            protocol,
            endpoint,
            chat,
            headers: BTreeMap::new(),
            rates: None,
        }
    }
}

pub(crate) fn validate(model: &Model) -> Result<(), crate::Failure> {
    validate_structure(model)?;
    if model.rates.as_ref().is_some_and(|rates| {
        [
            rates.input,
            rates.output,
            rates.cache_read,
            rates.cache_write,
        ]
        .iter()
        .any(|rate| !rate.is_finite() || *rate < 0.0)
    }) {
        return Err(crate::Failure::InvalidCatalog);
    }
    Ok(())
}

pub(crate) fn validate_structure(model: &Model) -> Result<(), crate::Failure> {
    let required = [
        &model.identity.provider,
        &model.identity.model,
        &model.identity.operation,
        &model.protocol,
        &model.endpoint,
        &model.name,
    ];
    let chat_valid = match (&model.chat, model.identity.operation.as_str()) {
        (Some(chat), "chat") => chat.context_window != Some(0),
        (None, op) => op != "chat",
        _ => false,
    };
    if required.iter().any(|s| s.is_empty())
        || model.input.iter().any(String::is_empty)
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
