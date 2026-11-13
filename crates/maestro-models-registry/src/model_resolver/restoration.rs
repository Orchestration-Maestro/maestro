//! Saved model restoration with current and available fallbacks.
use super::{CatalogError, ModelResolverOutput, label, selection::default_or_first};
use crate::ModelRegistry;
use maestro_models::Model;
use std::sync::{Arc, RwLock};
/// Borrowed operands for [`restore_model_from_session`].
#[derive(Clone, Copy)]
pub struct RestoreModelFromSessionOptions<'a> {
    /// Provider of the saved model.
    pub saved_provider: &'a str,
    /// ID of the saved model.
    pub saved_model_id: &'a str,
    /// Model to prefer when the saved one cannot be restored.
    pub current_model: Option<&'a Arc<RwLock<Model>>>,
    /// Whether to write restoration notices to the output streams.
    pub should_print_messages: bool,
    /// Catalog consulted for every lookup.
    pub model_registry: &'a ModelRegistry,
}
/// Restored or fallback model with the explanation of any fallback.
pub struct RestoreModelFromSessionResult {
    /// Restored or fallback descriptor, if any.
    pub model: Option<Arc<RwLock<Model>>>,
    /// Why a fallback was used; absent after a successful restore or with no model.
    pub fallback_message: Option<String>,
}
/// The saved model `provider`/`model_id` when the catalog has it with configured
/// authentication; absent when it is gone or has no credentials.
///
/// # Errors
/// Returns the authentication lookup failure.
pub fn restore_saved_model(
    model_registry: &ModelRegistry,
    provider: &str,
    model_id: &str,
) -> Result<Option<Arc<RwLock<Model>>>, CatalogError> {
    let Some(model) = model_registry.find(provider, model_id) else {
        return Ok(None);
    };
    Ok(model_registry.has_configured_auth(&model)?.then_some(model))
}

/// Restore the saved model when it exists with configured authentication.
///
/// Otherwise fall back to the current model without checking it, then to the first
/// available provider default or available model.
///
/// # Errors
/// Returns the first consumed authentication or availability failure.
pub fn restore_model_from_session(
    options: RestoreModelFromSessionOptions<'_>,
    output: &mut ModelResolverOutput<'_>,
) -> Result<RestoreModelFromSessionResult, CatalogError> {
    let RestoreModelFromSessionOptions {
        saved_provider,
        saved_model_id,
        current_model,
        should_print_messages: print,
        model_registry,
    } = options;
    let saved_label = format!("{saved_provider}/{saved_model_id}");
    if let Some(model) = restore_saved_model(model_registry, saved_provider, saved_model_id)? {
        if print {
            output.note(&format!("Restored model: {saved_label}"));
        }
        return Ok(RestoreModelFromSessionResult {
            model: Some(model),
            fallback_message: None,
        });
    }
    let reason = if model_registry
        .find(saved_provider, saved_model_id)
        .is_some()
    {
        "no auth configured"
    } else {
        "model no longer exists"
    };
    if print {
        output.warn(&format!(
            "Could not restore model {saved_label} ({reason})."
        ));
    }
    let fallback = match current_model {
        Some(model) => Some(Arc::clone(model)),
        None => default_or_first(&model_registry.get_available()?),
    };
    let Some(fallback) = fallback else {
        return Ok(RestoreModelFromSessionResult {
            model: None,
            fallback_message: None,
        });
    };
    if print {
        output.note(&format!("Falling back to: {}", label(&fallback)));
    }
    let fallback_message = format!(
        "Could not restore model {saved_label} ({reason}). Using {}.",
        label(&fallback)
    );
    Ok(RestoreModelFromSessionResult {
        model: Some(fallback),
        fallback_message: Some(fallback_message),
    })
}
