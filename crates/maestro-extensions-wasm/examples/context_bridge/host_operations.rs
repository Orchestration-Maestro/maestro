//! Factory-time observations through the author settings and package facades.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{ExtensionAPI, ExtensionResult, settings_manager::SettingsManager};

/// Reports raw settings before any command context or runner exists.
pub async fn observe(api: &ExtensionAPI) -> ExtensionResult<()> {
    let settings = SettingsManager::new(api.clone());
    let global = settings.get_global_settings().await?;
    let project = settings.get_project_settings().await?;
    api.events()
        .emit("host-operations", serde_json::json!([global, project]))
        .await
}
