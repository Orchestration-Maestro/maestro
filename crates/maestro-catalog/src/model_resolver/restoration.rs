//! Saved model restoration with current and available fallbacks.
use super::{ModelResolverOutput, label, selection::default_or_first};
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
/// Restore the saved model when it exists with configured authentication.
///
/// Otherwise fall back to the current model without checking it, then to the first
/// available provider default or available model.
pub fn restore_model_from_session(
    options: RestoreModelFromSessionOptions<'_>,
    output: &mut ModelResolverOutput<'_>,
) -> RestoreModelFromSessionResult {
    let RestoreModelFromSessionOptions {
        saved_provider,
        saved_model_id,
        current_model,
        should_print_messages: print,
        model_registry,
    } = options;
    let saved = model_registry.find(saved_provider, saved_model_id);
    let saved_label = format!("{saved_provider}/{saved_model_id}");
    if let Some(model) = saved
        .as_ref()
        .filter(|model| model_registry.has_configured_auth(model))
    {
        if print {
            output.note(&format!("Restored model: {saved_label}"));
        }
        return RestoreModelFromSessionResult {
            model: Some(Arc::clone(model)),
            fallback_message: None,
        };
    }
    let reason = if saved.is_some() {
        "no auth configured"
    } else {
        "model no longer exists"
    };
    if print {
        output.warn(&format!(
            "Could not restore model {saved_label} ({reason})."
        ));
    }
    let Some(fallback) = current_model
        .cloned()
        .or_else(|| default_or_first(&model_registry.get_available()))
    else {
        return RestoreModelFromSessionResult {
            model: None,
            fallback_message: None,
        };
    };
    if print {
        output.note(&format!("Falling back to: {}", label(&fallback)));
    }
    let fallback_message = format!(
        "Could not restore model {saved_label} ({reason}). Using {}.",
        label(&fallback)
    );
    RestoreModelFromSessionResult {
        model: Some(fallback),
        fallback_message: Some(fallback_message),
    }
}
