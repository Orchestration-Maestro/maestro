//! Application utility operations.

#[cfg(not(target_arch = "wasm32"))]
pub mod clipboard_native;
#[cfg(not(target_arch = "wasm32"))]
pub mod copying;
pub mod image_clipboard;
#[cfg(not(target_arch = "wasm32"))]
pub mod release_notes;

pub(crate) mod lock;
pub(crate) mod numbers;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod task;
pub(crate) mod uri;
