//! Controlled public chat qualification.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;
#[cfg(not(target_arch = "wasm32"))]
use crate::chat_qualification as qualification;

#[path = "chat_tool_choices/mod.rs"]
mod suite;
