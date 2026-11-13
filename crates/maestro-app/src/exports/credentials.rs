//! Existing credentials owners exposed at the application root.

pub use maestro_credentials::{
    ApiKeyCredential, AuthCredential, AuthStatus, AuthStorage, AuthStorageBackend,
    InMemoryAuthStorageBackend, OAuthCredential,
};

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_credentials::FileAuthStorageBackend;
