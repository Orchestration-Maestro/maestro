//! Application utility operations.

#[cfg(not(target_arch = "wasm32"))]
pub mod changelog;
#[cfg(not(target_arch = "wasm32"))]
pub mod clipboard;
pub mod clipboard_image;
#[cfg(not(target_arch = "wasm32"))]
pub mod clipboard_native;

pub(crate) mod lock;
pub(crate) mod numbers;
#[cfg(not(target_arch = "wasm32"))]
pub mod task;
pub(crate) mod uri;
