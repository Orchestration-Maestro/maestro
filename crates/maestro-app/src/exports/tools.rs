//! Existing tools owners exposed at the application root.

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_tools::{
    BashOperations, BashSpawnContext, BashSpawnHook, BashToolDetails, BashToolInput,
    BashToolOptions, create_bash_tool, create_bash_tool_definition, create_local_bash_operations,
    utils::get_shell_config,
};

pub use maestro_tools::{ToolDefinition, ToolRenderResultOptions};

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_tools::{ExecOptions, ExecResult};

pub use maestro_tools::{RenderDiffOptions, key_hint, key_text, raw_key_hint, render_diff};

/// Shared with the terminal frontend.
pub use maestro_tools::output::render_utils::format_read_line_range;

pub use maestro_tools::{VisualTruncateResult, truncate_to_visual_lines};

pub use maestro_tools::{TruncationOptions, TruncationResult, truncate_tail};

pub use maestro_tools::utils::image_convert::convert_to_png;

/// Image fitting and the file effects the command-line input needs.
pub use maestro_tools::utils::{ResizedImage, format_dimension_note, resize_image};

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_tools::utils::mime::detect_supported_image_mime_type_from_file;

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_tools::{
    EditToolOptions, ReadToolOptions, WriteToolOptions, create_edit_tool_definition,
    create_read_tool_definition, create_write_tool_definition,
};

/// Caller-local tool definitions and native shell execution.
pub mod local {
    #[cfg(not(target_arch = "wasm32"))]
    pub use maestro_tools::local::create_local_bash_operations;
}

pub use maestro_tools::define_tool;

pub use maestro_tools::{
    DEFAULT_MAX_BYTES, DEFAULT_MAX_LINES, format_size, truncate_head, truncate_line,
};

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_tools::{
    EditOperations, EditToolDetails, EditToolInput, FindOperations, FindToolDetails, FindToolInput,
    FindToolOptions, GrepOperations, GrepToolDetails, GrepToolInput, GrepToolOptions, LsOperations,
    LsToolDetails, LsToolInput, LsToolOptions, ReadOperations, ReadToolDetails, ReadToolInput,
    ToolsOptions, WriteOperations, WriteToolInput, create_coding_tools, create_edit_tool,
    create_find_tool, create_find_tool_definition, create_grep_tool, create_grep_tool_definition,
    create_ls_tool, create_ls_tool_definition, create_read_only_tools, create_read_tool,
    create_write_tool, with_file_mutation_queue,
};
