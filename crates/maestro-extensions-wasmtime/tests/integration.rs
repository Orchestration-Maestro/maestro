//! Native component execution and retained context integration tests.
#![cfg(all(test, not(target_arch = "wasm32")))]

mod abort_observers;
mod asynchronous_reentry;
mod canonical_api;
mod canonical_callbacks;
mod canonical_commands;
mod canonical_json;
mod canonical_schema;
mod canonical_theme;
mod canonical_ui;
mod component_events;
mod component_loading;
mod components;
mod exec_io;
mod execution_bridge;
mod factory_shutdown;
mod lifetimes;
mod listener_prefixes;
mod module_location;
mod provider_catalog;
mod retained_components;
mod retained_contexts;
mod startup;
mod synchronous_reentry;
mod wasi_access;
mod wasi_network;
mod wasi_output;
mod wasi_support;

#[path = "../../maestro-extensions/tests/support/session.rs"]
mod session;
mod support;

#[path = "../../maestro-extensions/tests/support/model.rs"]
mod model_support;

#[path = "../../maestro-extensions/tests/support/command_operations.rs"]
mod context_actions;

#[path = "../../maestro-test-terminal/src/manual_runtime.rs"]
#[allow(dead_code, reason = "Shared terminal fixture.")]
mod manual_runtime;

mod authored_stream_lifetimes;
mod authored_streams;
#[path = "../../maestro-test-terminal/src/recording_terminal.rs"]
#[allow(dead_code, reason = "Shared terminal fixture.")]
mod recording_terminal;

mod model_streams;

mod model_completions;

mod model_hooks;

mod model_finite;

mod model_hook_gates;
mod model_signal_selection;
