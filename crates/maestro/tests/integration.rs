//! Executable startup, package commands and session protocol qualification.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
mod entry_routes;
mod model_streams;
mod startup_modes;
mod support;
