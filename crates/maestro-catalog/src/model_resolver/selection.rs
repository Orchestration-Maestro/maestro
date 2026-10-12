//! Initial model choice from command line, scope, settings and availability.
use super::{
    CatalogError, DEFAULT_MODEL_PER_PROVIDER, ResolveCliModelOptions, ScopedModel,
    resolve_cli_model,
};
use crate::{DEFAULT_THINKING_LEVEL, ModelRegistry};
use maestro_models::{Model, ModelThinkingLevel};
use std::{
    cmp::Ordering,
    sync::{Arc, PoisonError, RwLock},
};
/// Borrowed operands for [`find_initial_model`].
pub struct FindInitialModelOptions<'a> {
    /// Provider named on the command line; ignored without a model.
    pub cli_provider: Option<&'a str>,
    /// Model reference named on the command line; ignored without a provider.
    pub cli_model: Option<&'a str>,
    /// Models the user limited the session to.
    pub scoped_models: &'a [ScopedModel],
    /// Whether an earlier session is being resumed.
    pub is_continuing: bool,
    /// Provider of the saved default model.
    pub default_provider: Option<&'a str>,
    /// ID of the saved default model.
    pub default_model_id: Option<&'a str>,
    /// Saved default effort.
    pub default_thinking_level: Option<ModelThinkingLevel>,
    /// Catalog consulted for every lookup.
    pub model_registry: &'a ModelRegistry,
}
/// Initial model and effort.
pub struct InitialModelResult {
    /// Chosen shared or detached descriptor, if any.
    pub model: Option<Arc<RwLock<Model>>>,
    /// Effort to start with.
    pub thinking_level: ModelThinkingLevel,
    /// Always absent; kept for symmetry with restoration.
    pub fallback_message: Option<String>,
}
/// Pick the starting model: command line, first scoped model, saved default, then availability.
///
/// A saved default is used without checking authentication; availability decides only after it.
///
/// # Errors
/// Returns the command-line resolution failure instead of terminating the process.
pub fn find_initial_model(
    options: FindInitialModelOptions<'_>,
    compare_model_ids: &dyn Fn(&str, &str) -> Ordering,
) -> Result<InitialModelResult, CatalogError> {
    let chosen = |model, thinking_level| {
        Ok(InitialModelResult {
            model,
            thinking_level,
            fallback_message: None,
        })
    };
    let registry = options.model_registry;
    if let (Some(cli_provider), Some(cli_model)) =
        (present(options.cli_provider), present(options.cli_model))
    {
        let resolved = resolve_cli_model(
            ResolveCliModelOptions {
                cli_provider: Some(cli_provider),
                cli_model: Some(cli_model),
                model_registry: registry,
            },
            compare_model_ids,
        );
        if let Some(error) = resolved.error {
            return Err(error);
        }
        if resolved.model.is_some() {
            return chosen(resolved.model, DEFAULT_THINKING_LEVEL);
        }
    }
    if let Some(first) = options
        .scoped_models
        .first()
        .filter(|_| !options.is_continuing)
    {
        let level = first.thinking_level.clone();
        return chosen(
            Some(Arc::clone(&first.model)),
            level
                .or(options.default_thinking_level)
                .unwrap_or(DEFAULT_THINKING_LEVEL),
        );
    }
    if let (Some(provider), Some(id)) = (
        present(options.default_provider),
        present(options.default_model_id),
    ) && let Some(found) = registry.find(provider, id)
    {
        return chosen(
            Some(found),
            options
                .default_thinking_level
                .unwrap_or(DEFAULT_THINKING_LEVEL),
        );
    }
    chosen(
        default_or_first(&registry.get_available()),
        DEFAULT_THINKING_LEVEL,
    )
}
/// An operand that is supplied and nonempty.
fn present(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.is_empty())
}
/// First available model matching the live provider defaults in table order, else the first.
pub(super) fn default_or_first(available: &[Arc<RwLock<Model>>]) -> Option<Arc<RwLock<Model>>> {
    let defaults = DEFAULT_MODEL_PER_PROVIDER
        .read()
        .unwrap_or_else(PoisonError::into_inner);
    defaults
        .iter()
        .find_map(|(provider, id)| {
            available.iter().find(|handle| {
                let model = handle.read().unwrap_or_else(PoisonError::into_inner);
                model.provider == *provider && model.id == *id
            })
        })
        .or(available.first())
        .cloned()
}
