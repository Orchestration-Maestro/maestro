//! Browser display utilities.
pub mod format;
pub mod i18n;

#[cfg(target_arch = "wasm32")]
pub(crate) mod download;

pub(crate) mod base64;

#[cfg(target_arch = "wasm32")]
pub(crate) mod clipboard;

/// Browser-local token kept in local storage.
#[cfg(target_arch = "wasm32")]
pub mod auth_token;
