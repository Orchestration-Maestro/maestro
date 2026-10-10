#![doc = include_str!("../README.md")]
//! Configured credential values, stored credentials, headers and login guidance.
mod credentials;
pub use credentials::auth_guidance::{
    format_no_api_key_found_message, format_no_model_selected_message,
    format_no_models_available_message, get_provider_login_help,
};
#[cfg(not(target_arch = "wasm32"))]
pub use credentials::auth_storage::FileAuthStorageBackend;
pub use credentials::auth_storage::{
    ApiKeyCredential, AsyncLockUpdate, AuthCredential, AuthSource, AuthStatus, AuthStorage,
    AuthStorageBackend, AuthStorageData, AuthStorageError, AuthStorageFuture,
    InMemoryAuthStorageBackend, LockUpdate, OAuthCredential,
};
#[cfg(not(target_arch = "wasm32"))]
pub use credentials::resolve_config_value::ProcessConfigValueOperations;
pub use credentials::resolve_config_value::{
    ConfigValueOperations, clear_config_value_cache, resolve_config_value,
    resolve_config_value_or_throw, resolve_config_value_uncached, resolve_headers,
    resolve_headers_or_throw,
};
