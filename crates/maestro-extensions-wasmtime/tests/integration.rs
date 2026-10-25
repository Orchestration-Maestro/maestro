//! Native component execution and retained context integration tests.
#![cfg(all(test, not(target_arch = "wasm32")))]

mod canonical_api;
mod canonical_callbacks;
mod canonical_commands;
mod canonical_json;
mod canonical_schema;
mod canonical_ui;
mod execution_bridge;
mod lifetimes;
mod listener_prefixes;
mod retained_components;
mod retained_contexts;
mod startup;
mod synchronous_reentry;

#[path = "../../maestro-extensions/tests/support/session.rs"]
mod session;
mod support;

#[path = "../../maestro-extensions/tests/support/model.rs"]
mod model_support;

#[path = "../../maestro-extensions/tests/support/command_operations.rs"]
mod context_actions;
