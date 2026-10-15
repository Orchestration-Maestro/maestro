//! Browser-owned attachment content effects.
pub mod attachment_content;

/// Cancel-on-drop browser timers.
pub mod timer;

/// Native sandbox values.
#[cfg(target_arch = "wasm32")]
pub mod sandbox_value;
