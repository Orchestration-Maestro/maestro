//! Executable startup, package commands and session protocol qualification.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
mod component_stdout;
mod custom_message_provider;
mod entry_routes;
mod model_streams;
mod native_io;
mod qna_example;
mod startup_modes;
mod support;
pub mod tools_example;
