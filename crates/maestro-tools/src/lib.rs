//! Tool definitions, the `read`, `write`, `edit`, `ls`, `find`, `grep` and native `bash` tools,
//! the file mutation queue, output and inline-image helpers and word-level edit diff rendering
//! for Maestro tools. Native [tools](registry) factories select individual tools and ordered presets.
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
//! use maestro_tools::utils::images::{ImageResizeOptions, resize_image};
//!
//! let image = ImageContent {
//!     data: "R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7".into(),
//!     mime_type: "image/gif".into(),
//! };
//! let result = resize_image(&image, ImageResizeOptions::default()).unwrap();
//! assert_eq!((result.width, result.height), (1, 1));
//! assert!(!result.was_resized);
//! ```

pub mod changes;
#[cfg(not(target_arch = "wasm32"))]
pub mod editing;
#[cfg(not(target_arch = "wasm32"))]
pub mod mutations;
#[cfg(not(target_arch = "wasm32"))]
pub mod previews;
pub use changes::{RenderDiffOptions, render_diff};
pub mod keybinding_hints;
pub mod output;
#[cfg(not(target_arch = "wasm32"))]
pub mod path_utils;
#[cfg(not(target_arch = "wasm32"))]
pub mod process;
#[cfg(not(target_arch = "wasm32"))]
pub mod reading;
#[cfg(not(target_arch = "wasm32"))]
pub mod registry;
pub mod renderers;
pub mod result_renderer;
pub mod retained;
#[cfg(not(target_arch = "wasm32"))]
pub mod search;
pub mod tool_definition_wrapper;
pub mod types;
#[cfg(not(target_arch = "wasm32"))]
pub use registry::{
    ALL_TOOL_NAMES, Tool, ToolDef, ToolDefRecord, ToolName, ToolsOptions, ToolsOptionsRecord,
    create_all_tool_definitions, create_all_tools, create_coding_tool_definitions,
    create_coding_tools, create_read_only_tool_definitions, create_read_only_tools, create_tool,
    create_tool_definition,
};
pub use renderers::{ToolRenderResultBinding, ToolRendererBinding};
pub use result_renderer::PreparedToolRenderResult;
pub use retained::ToolRendererState;
pub mod utils;
#[cfg(not(target_arch = "wasm32"))]
pub mod writing;

#[cfg(not(target_arch = "wasm32"))]
pub use editing::{
    EditOperations, EditRenderState, EditToolDetails, EditToolInput, EditToolOptions,
    create_edit_tool, create_edit_tool_definition,
};
pub use keybinding_hints::{key_hint, key_text, raw_key_hint};
#[cfg(not(target_arch = "wasm32"))]
pub use mutations::with_file_mutation_queue;
pub use output::limits::{
    DEFAULT_MAX_BYTES, DEFAULT_MAX_LINES, TruncateLineResult, TruncationBy, TruncationOptions,
    TruncationResult, format_size, truncate_head, truncate_line, truncate_tail,
};
#[cfg(not(target_arch = "wasm32"))]
pub use path_utils::{expand_path, resolve_read_path, resolve_to_cwd};
#[cfg(not(target_arch = "wasm32"))]
pub use process::{
    command::{
        BashError, BashExecOptions, BashExecResult, BashExecution, BashOperations, BashRenderState,
        BashSpawnContext, BashSpawnHook, BashToolDetails, BashToolInput, BashToolOptions,
        BashToolOptionsRecord, create_bash_tool, create_bash_tool_definition,
        create_local_bash_operations,
    },
    commands::{ExecOptions, ExecResult, exec_command},
    delay::{SleepError, sleep},
};
#[cfg(not(target_arch = "wasm32"))]
pub use reading::{
    ReadContext, ReadOperations, ReadToolDetails, ReadToolInput, ReadToolOptions, create_read_tool,
    create_read_tool_definition,
};
#[cfg(not(target_arch = "wasm32"))]
pub use search::directory::{
    LsOperations, LsToolDetails, LsToolInput, LsToolOptions, create_ls_tool,
    create_ls_tool_definition,
};
#[cfg(not(target_arch = "wasm32"))]
pub use search::discovery::{
    FindGlobOptions, FindOperations, FindToolDetails, FindToolInput, FindToolOptions,
    create_find_tool, create_find_tool_definition,
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
pub use writing::{
    WriteOperations, WriteToolInput, WriteToolOptions, create_write_tool,
    create_write_tool_definition,
};

#[cfg(not(target_arch = "wasm32"))]
pub use search::acquisition::{SearchTool, ToolsManager, ToolsManagerOutput};

#[cfg(not(target_arch = "wasm32"))]
pub use process::executor::{BashExecutorOptions, BashResult, execute_bash_with_operations};

pub use output::preview::{VisualTruncateResult, truncate_to_visual_lines};
#[cfg(not(target_arch = "wasm32"))]
pub use search::matching::{
    GrepOperations, GrepToolDetails, GrepToolInput, GrepToolOptions, create_grep_tool,
    create_grep_tool_definition,
};

/// Tool factories retaining callbacks on the caller's thread.
pub mod local {
    #[cfg(not(target_arch = "wasm32"))]
    pub use crate::registry::local::*;
    pub use crate::tool_definition_wrapper::local::*;
    pub use crate::types::local::*;
}
