//! Controlled response identity qualification.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;
#[path = "history_handoffs/mod.rs"]
mod suite;
#[cfg(not(target_arch = "wasm32"))]
use crate::response_qualification as qualification;
