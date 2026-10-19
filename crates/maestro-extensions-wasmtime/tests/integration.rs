//! Native component execution and retained context integration tests.
#![cfg(all(test, not(target_arch = "wasm32")))]

mod canonical_api;
mod canonical_callbacks;
mod canonical_commands;
mod canonical_json;
mod execution_bridge;
mod listener_prefixes;
mod retained_components;
mod startup;

#[path = "../../maestro-extensions/tests/support/session.rs"]
mod session;
mod support;

#[path = "../../maestro-extensions/tests/support/model.rs"]
mod model_support;
