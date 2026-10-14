//! Application utility operations.

#[cfg(not(target_arch = "wasm32"))]
pub mod clipboard;
pub mod clipboard_image;
#[cfg(not(target_arch = "wasm32"))]
pub mod clipboard_native;
