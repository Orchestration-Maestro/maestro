//! Browser display utilities.
pub mod formatting;
pub mod localization;

#[cfg(target_arch = "wasm32")]
pub(crate) mod download;

pub(crate) mod base64;

#[cfg(target_arch = "wasm32")]
pub(crate) mod clipboard;
