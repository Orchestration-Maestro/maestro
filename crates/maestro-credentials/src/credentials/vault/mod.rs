//! Accepted stored credentials over a replaceable raw-text backend.
mod backend;
mod cloudflare;
mod records;
mod refresh;
mod resolution;
mod state;

#[cfg(not(target_arch = "wasm32"))]
pub use backend::FileAuthStorageBackend;
pub use backend::{
    AsyncLockUpdate, AuthStorageBackend, AuthStorageFuture, InMemoryAuthStorageBackend, LockUpdate,
};
pub use records::{
    ApiKeyCredential, AuthCredential, AuthSource, AuthStatus, AuthStorageData, AuthStorageError,
    OAuthCredential,
};
pub use state::AuthStorage;

#[cfg(test)]
mod tests;
