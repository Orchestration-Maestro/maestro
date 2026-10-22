//! Shared integration-test executable.
#![cfg(test)]

mod callback_delivery;
mod channel_delivery;
mod command_contexts;
mod extension_actions;
mod extension_contexts;
mod extension_events;
mod extension_path_loading;
mod hook_boundaries;
mod hook_dispatch;
mod hook_exports;
mod hook_transforms;
mod local_registration;
mod registered_tools;
mod registration_queries;
mod ui_capabilities;

#[cfg(not(target_arch = "wasm32"))]
mod support;

#[cfg(not(target_arch = "wasm32"))]
#[path = "support/local.rs"]
mod local_support;

#[cfg(not(target_arch = "wasm32"))]
#[path = "support/runtime.rs"]
mod runtime_support;

#[cfg(not(target_arch = "wasm32"))]
#[path = "support/model.rs"]
mod model_support;

#[cfg(not(target_arch = "wasm32"))]
#[path = "support/ui_host.rs"]
mod ui_host;

#[cfg(not(target_arch = "wasm32"))]
#[path = "support/context_actions.rs"]
mod context_actions;

#[cfg(not(target_arch = "wasm32"))]
#[path = "support/error.rs"]
mod error_support;

#[cfg(not(target_arch = "wasm32"))]
#[path = "support/tool.rs"]
mod tool_support;

#[cfg(not(target_arch = "wasm32"))]
#[path = "support/limited_host.rs"]
mod limited_host;

#[cfg(unix)]
#[path = "support/module_loader.rs"]
mod module_loader;

#[cfg(not(target_arch = "wasm32"))]
use display_support::manual_runtime;

#[cfg(not(target_arch = "wasm32"))]
#[path = "../../maestro-test-terminal/tests/support/recording_terminal.rs"]
#[allow(dead_code, reason = "Shared terminal fixture.")]
mod recording_terminal;

#[cfg(not(target_arch = "wasm32"))]
#[path = "../../maestro-tools/tests/display_support/mod.rs"]
#[allow(
    dead_code,
    reason = "This program uses part of the tools display support."
)]
mod display_support;

#[cfg(not(target_arch = "wasm32"))]
#[path = "support/session.rs"]
mod session;
