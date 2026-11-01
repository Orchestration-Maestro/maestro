//! Shared integration-test executable.

mod agent_shell_execution;
mod agent_shell_native;
mod agent_shell_progress;
#[cfg(not(target_arch = "wasm32"))]
mod agent_shell_support;
mod authored_schemas;
mod cancellable_delay;
mod command_execution;
mod diff_rendering;
mod diff_support;
mod direct_shell_execution;
mod display_cases;
mod display_context;
mod display_helpers;
mod display_palette;
mod display_support;
mod edit_execution;
#[cfg(not(target_arch = "wasm32"))]
mod edit_observers;
mod edit_preparation;
mod edit_preview;
mod edit_render_support;
mod edit_rendering;
mod edit_snapshot_matching;
mod edit_support;
mod execution_affinity;
mod exif_orientation_behavior;
mod factory_support;
mod find_execution;
mod find_rendering;
#[cfg(not(target_arch = "wasm32"))]
mod find_support;
mod grep_execution;
mod grep_rendering;
#[cfg(not(target_arch = "wasm32"))]
mod grep_support;
mod image_mime_detection;
mod image_preparation;
mod local_shell_execution;
mod local_shell_lifecycle;
mod loop_support;
mod ls_execution;
mod ls_rendering;
mod ls_support;
mod maestro_tool_factories;
mod path_spelling;
mod process_support;
mod queue_ordering;
mod read_execution;
mod read_loop;
mod read_path_behavior;
mod read_rendering;
mod read_support;
mod search_tool_lookup;
mod shell_tree_cleanup;
mod startup_environment;
mod streamed_output;
mod support;
mod tool_definition_contract;
mod tool_record_views;
mod truncation_behavior;
mod write_behavior;
mod write_rendering;
mod write_support;

mod visual_tail;

mod bash_operation_affinity;
mod bash_rendering;

mod renderer_projection;

mod renderer_projection_support;

#[path = "support/runtime.rs"]
mod runtime;
