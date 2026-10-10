//! Authentication guidance using a supplied documentation root.
/// Format login help without reading documentation files.
#[must_use]
pub fn get_provider_login_help(docs_path: &str) -> String {
    format!(
        "Use /login to log into a provider via OAuth or API key. See:\n  {}\n  {}",
        maestro_path::join(&[docs_path, "providers.md"]),
        maestro_path::join(&[docs_path, "models.md"])
    )
}
/// Format the message for an empty available-model list.
#[must_use]
pub fn format_no_models_available_message(docs_path: &str) -> String {
    format!(
        "No models available. {}",
        get_provider_login_help(docs_path)
    )
}
/// Format guidance for choosing a model after authentication.
#[must_use]
pub fn format_no_model_selected_message(docs_path: &str) -> String {
    format!(
        "No model selected.\n\n{}\n\nThen use /model to select a model.",
        get_provider_login_help(docs_path)
    )
}
/// Format a missing-key message, replacing only the exact unknown-provider sentinel.
#[must_use]
pub fn format_no_api_key_found_message(provider: &str, docs_path: &str) -> String {
    let provider = if provider == "unknown" {
        "the selected model"
    } else {
        provider
    };
    format!(
        "No API key found for {provider}.\n\n{}",
        get_provider_login_help(docs_path)
    )
}
