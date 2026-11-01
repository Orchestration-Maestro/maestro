//! Tool definitions, the `read`, `write`, `edit`, `ls`, `find`, `grep` and native `bash` tools,
//! the file mutation queue, output and inline-image helpers and word-level edit diff rendering
//! for Maestro tools. Native [tools] factories select individual tools and ordered presets.
//!
//! Native bash definitions retain `BashRenderState` for call/result timing and
//! five-row visual previews. [`truncate_to_visual_lines`] also selects wrapped
//! output independently on every target.
//!
//! ```
//! use maestro_tools::{TruncationOptions, truncate_head};
//!
//! let selected = truncate_head("first\nlast", TruncationOptions {
//!     max_lines: Some(1),
//!     ..TruncationOptions::default()
//! });
//! assert_eq!(selected.content, "first");
//! assert_eq!(selected.total_lines, 2);
//! ```

//! Image preparation is also independently callable:
//!
//! ```
//! use maestro_models::ImageContent;
//! use maestro_tools::utils::image_resize::{ImageResizeOptions, resize_image};
//!
//! let image = ImageContent {
//!     data: "R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7".into(),
//!     mime_type: "image/gif".into(),
//! };
//! let result = resize_image(&image, ImageResizeOptions::default()).unwrap();
//! assert_eq!((result.width, result.height), (1, 1));
//! assert!(!result.was_resized);
//! ```

pub mod diff;
#[cfg(not(target_arch = "wasm32"))]
pub mod edit;
#[cfg(not(target_arch = "wasm32"))]
pub mod edit_diff;
#[cfg(not(target_arch = "wasm32"))]
pub mod file_mutation_queue;
pub use diff::{RenderDiffOptions, render_diff};
pub mod keybinding_hints;
pub mod output;
#[cfg(not(target_arch = "wasm32"))]
pub mod path_utils;
#[cfg(not(target_arch = "wasm32"))]
pub mod process;
#[cfg(not(target_arch = "wasm32"))]
pub mod read;
pub mod renderer_binding;
pub mod renderer_state;
pub mod result_renderer;
#[cfg(not(target_arch = "wasm32"))]
pub mod search;
pub mod tool_definition_wrapper;
#[cfg(not(target_arch = "wasm32"))]
pub mod tools;
pub mod types;
pub use renderer_binding::{ToolRenderResultBinding, ToolRendererBinding};
pub use renderer_state::ToolRendererState;
pub use result_renderer::PreparedToolRenderResult;
#[cfg(not(target_arch = "wasm32"))]
pub use tools::{
    ALL_TOOL_NAMES, Tool, ToolDef, ToolDefRecord, ToolName, ToolsOptions, ToolsOptionsRecord,
    create_all_tool_definitions, create_all_tools, create_coding_tool_definitions,
    create_coding_tools, create_read_only_tool_definitions, create_read_only_tools, create_tool,
    create_tool_definition,
};
pub mod utils;
#[cfg(not(target_arch = "wasm32"))]
pub mod write;

#[cfg(not(target_arch = "wasm32"))]
pub use edit::{
    EditOperations, EditRenderState, EditToolDetails, EditToolInput, EditToolOptions,
    create_edit_tool, create_edit_tool_definition,
};
#[cfg(not(target_arch = "wasm32"))]
pub use file_mutation_queue::with_file_mutation_queue;
pub use keybinding_hints::{key_hint, key_text, raw_key_hint};
#[cfg(not(target_arch = "wasm32"))]
pub use output::render_parts::expand_hint;
pub use output::truncate::{
    DEFAULT_MAX_BYTES, DEFAULT_MAX_LINES, TruncateLineResult, TruncationBy, TruncationOptions,
    TruncationResult, format_size, truncate_head, truncate_line, truncate_tail,
};
#[cfg(not(target_arch = "wasm32"))]
pub use path_utils::{expand_path, resolve_read_path, resolve_to_cwd};
#[cfg(not(target_arch = "wasm32"))]
pub use process::{
    bash::{
        BashError, BashExecOptions, BashExecResult, BashExecution, BashOperations, BashRenderState,
        BashSpawnContext, BashSpawnHook, BashToolDetails, BashToolInput, BashToolOptions,
        BashToolOptionsRecord, create_bash_tool, create_bash_tool_definition,
        create_local_bash_operations,
    },
    exec::{ExecOptions, ExecResult, exec_command},
    sleep::{SleepError, sleep},
};
#[cfg(not(target_arch = "wasm32"))]
pub use read::{
    ReadContext, ReadOperations, ReadToolDetails, ReadToolInput, ReadToolOptions, create_read_tool,
    create_read_tool_definition,
};
#[cfg(not(target_arch = "wasm32"))]
pub use search::find::{
    FindGlobOptions, FindOperations, FindToolDetails, FindToolInput, FindToolOptions,
    create_find_tool, create_find_tool_definition,
};
#[cfg(not(target_arch = "wasm32"))]
pub use search::ls::{
    LsOperations, LsToolDetails, LsToolInput, LsToolOptions, create_ls_tool,
    create_ls_tool_definition,
};
pub use tool_definition_wrapper::{
    create_tool_definition_from_agent_tool, wrap_tool_definition, wrap_tool_definitions,
};
pub use types::{
    ExecuteToolDefinition, ToolContextFactory, ToolDefinition, ToolDefinitionRecord, ToolExecution,
    ToolRenderCall, ToolRenderContext, ToolRenderEnvironment, ToolRenderPhase, ToolRenderResult,
    ToolRenderResultCallback, ToolRenderResultOptions, ToolRenderShell, define_tool,
};
#[cfg(not(target_arch = "wasm32"))]
pub use write::{
    WriteOperations, WriteToolInput, WriteToolOptions, create_write_tool,
    create_write_tool_definition,
};

#[cfg(not(target_arch = "wasm32"))]
pub use search::tools_manager::{SearchTool, ToolsManager, ToolsManagerOutput};

#[cfg(not(target_arch = "wasm32"))]
pub use process::bash_executor::{BashExecutorOptions, BashResult, execute_bash_with_operations};

pub use output::visual_truncate::{VisualTruncateResult, truncate_to_visual_lines};
#[cfg(not(target_arch = "wasm32"))]
pub use search::grep::{
    GrepOperations, GrepToolDetails, GrepToolInput, GrepToolOptions, create_grep_tool,
    create_grep_tool_definition,
};

/// Tool factories retaining callbacks on the caller's thread.
pub mod local {
    pub use crate::tool_definition_wrapper::local::*;
    #[cfg(not(target_arch = "wasm32"))]
    pub use crate::tools::local::*;
    pub use crate::types::local::*;
}
