//! Accepted stored credentials over a replaceable raw-text backend.
mod backend;
mod refresh;
mod resolution;
mod state;
mod types;

#[cfg(not(target_arch = "wasm32"))]
pub use backend::FileAuthStorageBackend;
pub use backend::{
    AsyncLockUpdate, AuthStorageBackend, AuthStorageFuture, InMemoryAuthStorageBackend, LockUpdate,
};
pub use state::AuthStorage;
pub use types::{
    ApiKeyCredential, AuthCredential, AuthSource, AuthStatus, AuthStorageData, AuthStorageError,
    OAuthCredential,
};

#[cfg(test)]
mod tests;
