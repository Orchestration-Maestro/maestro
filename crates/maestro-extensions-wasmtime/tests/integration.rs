//! Native component execution and retained context integration tests.
#![cfg(all(test, not(target_arch = "wasm32")))]
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

mod abort_observers;
mod asynchronous_reentry;
mod callback_lifetimes;
mod callback_ordering;
mod callback_results;
mod canonical_api;
mod canonical_callbacks;
mod canonical_commands;
mod canonical_json;
mod canonical_schema;
mod canonical_theme;
mod canonical_ui;
mod canonical_widgets;
mod compilation_cache;
mod completion_providers;
mod component_cache_location;
mod component_callbacks;
mod component_events;
mod component_loading;

mod components;
mod context_bridge;
mod context_retention;
mod editor_factories;
mod exec_io;
mod execution_bridge;
mod factory_capabilities;
mod factory_shutdown;
mod hook_diagnostics;
mod lifetimes;
mod listener_prefixes;
mod load_time_contributions;
mod module_location;
mod native_helpers;
mod native_io;
mod provider_catalog;
mod replacement_callbacks;
mod replacement_host;
mod retained_components;
mod retained_contexts;
mod runtime_retention;
mod startup;
mod synchronous_reentry;
mod wasi_access;
mod wasi_network;
mod wasi_output;
mod wasi_support;

#[path = "../../maestro-extensions/tests/support/session.rs"]
mod session;
mod support;

#[path = "support/example_process.rs"]
mod example_process;

mod model_support {
    pub use maestro_extensions::test_support::model;
}

/// The shared silent command operations the adapter's witnesses bind.
mod context_actions {
    pub use maestro_extensions::test_support::silent_command_operations;
}

use maestro_tui::test_support::manual_runtime;

mod authored_stream_lifetimes;
mod authored_streams;
use maestro_tui::test_support::recording_terminal;

mod model_streams;

mod model_completions;

mod model_hooks;

mod model_finite;

mod model_hook_gates;
mod model_signal_selection;

mod tui_capabilities;

mod custom_ui;
mod model_lifetimes;

mod foreign_schema;
mod model_protocol_routes;
mod oauth;

mod model_registered;

/// The production artifact factory over toolkit widgets and a fixed installation version.
fn artifact(path: String) -> maestro_extensions::ExtensionFactory<maestro_extensions::Local> {
    maestro_extensions_wasmtime::artifact_factory(
        path,
        support::widgets(),
        maestro_extensions_wasmtime::Installation {
            version: "installed-component-host".into(),
            agent_dir: maestro_test_process::run_agent_directory().unwrap_or_default(),
        },
    )
}
