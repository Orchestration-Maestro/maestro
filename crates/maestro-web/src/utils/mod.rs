//! Browser display utilities.
pub mod format;
pub mod i18n;

#[cfg(target_arch = "wasm32")]
pub(crate) mod download;
