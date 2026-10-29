//! Native component execution and retained context integration tests.
#![cfg(all(test, not(target_arch = "wasm32")))]

mod abort_observers;
mod asynchronous_reentry;
mod auto_commit_on_exit_example;
mod bash_spawn_hook_example;
mod border_status_editor_example;
mod built_in_tool_renderer;
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
mod component_events;
mod component_loading;
mod components;
mod doom_engine_errors;
mod doom_overlay_files;
mod doom_wad;
mod editor_factories;
mod exec_io;
mod execution_bridge;
mod extension_guide;
mod factory_capabilities;
mod factory_shutdown;
mod git_checkpoint_example;
mod handoff_example;
mod inline_bash_example;
mod interactive_shell_example;
mod lifetimes;
mod listener_prefixes;
mod load_time_contributions;
mod minimal_mode_example;
mod module_location;
mod native_helpers;
mod native_io;
mod overlay_qa_tests_example;
mod overlay_test_example;
mod project_rules_example;
mod provider_catalog;
mod retained_components;
mod retained_contexts;
mod startup;
mod summarize_example;
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

mod tool_override_example;
mod tui_capabilities;

mod custom_compaction_example;
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
            agent_dir: "/controlled-agent".into(),
        },
    )
}
