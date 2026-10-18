//! Existing tools owners exposed at the application root.

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_tools::{
    BashOperations, BashRenderState, BashSpawnContext, BashSpawnHook, BashToolDetails,
    BashToolInput, BashToolOptions, create_bash_tool, create_bash_tool_definition,
    create_local_bash_operations, utils::shell::get_shell_config,
};

pub use maestro_tools::output::render_utils::{ToolRenderResultLike, get_text_output};

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_tools::{EditRenderState, ToolDef};

pub use maestro_tools::{
    ToolDefinition, ToolRenderCall, ToolRenderContext, ToolRenderEnvironment, ToolRenderPhase,
    ToolRenderResult, ToolRenderResultBinding, ToolRenderResultCallback, ToolRenderResultOptions,
    ToolRenderShell, ToolRendererBinding, ToolRendererState,
};

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_tools::process::exec::{ExecOptions, ExecResult, exec_command};

pub use maestro_tools::{RenderDiffOptions, key_hint, key_text, raw_key_hint, render_diff};

/// Shared with the terminal frontend.
pub use maestro_tools::output::render_utils::format_read_line_range;

pub use maestro_tools::{VisualTruncateResult, truncate_to_visual_lines};

pub use maestro_tools::output::render_utils::strip_ansi;

pub use maestro_tools::{TruncationOptions, TruncationResult, truncate_tail};

pub use maestro_tools::keybinding_hints::plain_key_hint;

pub use maestro_tools::output::render_utils::num;

pub use maestro_tools::utils::image_convert::{ConvertedImage, convert_to_png};

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_tools::{
    EditToolOptions, ReadToolOptions, WriteToolOptions, create_edit_tool_definition,
    create_read_tool_definition, create_write_tool_definition,
};
