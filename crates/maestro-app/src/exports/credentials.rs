//! Existing credentials owners exposed at the application root.

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_credentials::ProcessConfigValueOperations;
pub use maestro_credentials::{
    ApiKeyCredential, AuthCredential, AuthSource, AuthStatus, AuthStorage, AuthStorageBackend,
    AuthStorageData, ConfigValueOperations, InMemoryAuthStorageBackend, OAuthCredential,
};

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_credentials::FileAuthStorageBackend;

pub use maestro_credentials::format_no_models_available_message;
