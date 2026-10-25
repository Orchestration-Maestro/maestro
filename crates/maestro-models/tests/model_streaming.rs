//! Controlled streaming qualification.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;
#[cfg(not(target_arch = "wasm32"))]
use crate::response_qualification;

#[path = "model_streaming/mod.rs"]
mod suite;
