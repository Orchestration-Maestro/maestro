//! Retained browser dialogs.
pub mod attachment_overlay;

/// Required provider key prompt.
#[cfg(target_arch = "wasm32")]
pub mod api_key_prompt_dialog;
/// Custom provider add and edit dialog.
#[cfg(target_arch = "wasm32")]
pub mod custom_provider_dialog;
/// Native modal backdrop, panel and the header, content and footer parts.
#[cfg(target_arch = "wasm32")]
pub mod dialog;
/// Opened dialogs that keep themselves mounted until closed.
#[cfg(target_arch = "wasm32")]
pub mod dialog_base;
/// Model picker over the catalog and custom providers.
#[cfg(any(target_arch = "wasm32", test))]
pub mod model_selector_dialog;
/// Persistent storage permission request.
#[cfg(target_arch = "wasm32")]
pub mod persistent_storage_dialog;
/// One-line text prompt settled by any close.
#[cfg(target_arch = "wasm32")]
pub mod prompt_dialog;
/// Provider keys and custom servers settings tab.
#[cfg(target_arch = "wasm32")]
pub mod providers_models_tab;
/// Saved conversation list.
#[cfg(target_arch = "wasm32")]
pub mod session_list_dialog;
/// Settings dialog and its tabs.
#[cfg(target_arch = "wasm32")]
pub mod settings_dialog;
