//! Shared integration-test executable.
#![cfg(test)]

mod custom_and_skill_messages;
mod extension_dialogs;
mod extension_editor;
#[path = "support/recorded_keys/filter_mode.rs"]
mod filter_mode;
mod footer_rendering;
#[path = "footer_support/mod.rs"]
mod footer_support;
#[path = "support/keybindings.rs"]
mod keybindings;
mod login_dialog;
mod login_provider_admission;
mod message_rendering;
mod model_selection;
mod model_selector_support;
#[path = "support/occurrences.rs"]
mod occurrences;
#[path = "message_support/palette.rs"]
mod palette;
mod provider_auth_selection;
#[path = "support/recorded_keys/registry.rs"]
mod recorded_registry;
mod resource_configuration;
#[path = "support/runtime_spy.rs"]
mod runtime_spy;
mod scoped_model_selection;
#[cfg(not(target_arch = "wasm32"))]
#[path = "../../maestro-packages/tests/scratch/mod.rs"]
mod scratch;
#[path = "support/selector.rs"]
mod selector;
mod selector_failure_support;
mod session_picker;
mod session_picker_scopes;
#[path = "support/session_picker.rs"]
mod session_picker_support;
#[cfg(not(target_arch = "wasm32"))]
#[path = "../../maestro-theme/tests/live/document.rs"]
mod theme_document;
// The session picker's helpers are compiled once, here and in the library tests that include it.
use registry::input_support;
use session_picker_support::{bindings, manual_runtime, published, recording_terminal, registry};
mod session_rename_and_delete;
mod session_search;
mod settings_display;
mod settings_selection;
mod settings_selection_support;
mod shared_components;
mod shared_session_content;
mod shared_tree_projection;
mod shell_rows;
mod simple_selector_cases;
mod simple_selectors;
#[path = "../../maestro-catalog/tests/support/subprocess.rs"]
mod subprocess;
mod tool_rows;
#[path = "support/tree.rs"]
mod tree;
mod tree_folding_and_labels;
mod tree_folding_and_search;
mod tree_rendering;
mod tree_selection;
