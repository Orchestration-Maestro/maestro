//! Existing clipboard owners exposed at the application root.

#[cfg(not(target_arch = "wasm32"))]
pub use crate::utils::clipboard::{copy_to_clipboard, copy_to_clipboard_with};
