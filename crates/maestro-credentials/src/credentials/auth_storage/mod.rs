//! Accepted stored credentials over a replaceable raw-text backend.
mod backend;
mod cloudflare;
mod refresh;
mod resolution;
mod state;
mod types;

pub use backend::{
    AsyncLockUpdate, AuthStorageBackend, AuthStorageFuture, InMemoryAuthStorageBackend, LockUpdate,
};
#[cfg(not(target_arch = "wasm32"))]
pub use backend::{FileAuthStorageBackend, write_auth_file};
pub use state::AuthStorage;
pub use types::{
    ApiKeyCredential, AuthCredential, AuthSource, AuthStatus, AuthStorageData, AuthStorageError,
    OAuthCredential,
};

#[cfg(test)]
mod tests;
