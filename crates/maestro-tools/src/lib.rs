//! Tool definitions, the `read`, `write`, `edit`, `ls`, `find`, `grep` and native `bash` tools,
//! the file mutation queue, output and inline-image helpers and word-level edit diff rendering
//! for Maestro tools. Native `tools` factories select individual tools and ordered presets.
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
//! use maestro_tools::utils::{ImageResizeOptions, resize_image};
//!
//! let image = ImageContent {
//!     data: "R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7".into(),
//!     mime_type: "image/gif".into(),
//! };
//! let result = resize_image(&image, ImageResizeOptions::default()).unwrap();
//! assert_eq!((result.width, result.height), (1, 1));
//! assert!(!result.was_resized);
//! ```

pub(crate) mod diff;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod edit;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod edit_diff;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod file_mutation_queue;
pub use diff::*;
#[cfg(not(target_arch = "wasm32"))]
pub use edit::*;
#[cfg(not(target_arch = "wasm32"))]
pub use edit_diff::*;
#[cfg(not(target_arch = "wasm32"))]
pub use file_mutation_queue::*;
pub mod keybinding_hints;
pub mod output;
#[cfg(not(target_arch = "wasm32"))]
pub mod path_utils;
#[cfg(not(target_arch = "wasm32"))]
pub mod process;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod read;
#[cfg(not(target_arch = "wasm32"))]
pub use read::*;
pub(crate) mod renderer_binding;
pub(crate) mod renderer_state;
pub mod result_renderer;
#[cfg(not(target_arch = "wasm32"))]
pub mod search;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod tools;
#[cfg(not(target_arch = "wasm32"))]
pub use search::find::{
    FindGlobOptions, FindOperations, FindToolDetails, FindToolInput, FindToolOptions,
    create_find_tool, create_find_tool_definition,
};
#[cfg(not(target_arch = "wasm32"))]
pub use search::grep::{
    GrepOperations, GrepToolDetails, GrepToolInput, GrepToolOptions, create_grep_tool,
    create_grep_tool_definition,
};
#[cfg(not(target_arch = "wasm32"))]
pub use search::ls::{
    LsOperations, LsToolDetails, LsToolInput, LsToolOptions, create_ls_tool,
    create_ls_tool_definition,
};
pub mod tool_definition_wrapper;
pub mod types;
pub use renderer_binding::*;
pub use renderer_state::*;
pub use result_renderer::PreparedToolRenderResult;
#[cfg(not(target_arch = "wasm32"))]
pub use tools::*;
pub mod utils;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod write;
#[cfg(not(target_arch = "wasm32"))]
pub use write::*;

pub use keybinding_hints::{
    expand_hint, format_key_text, key_display_text, key_hint, key_text, raw_key_hint,
};
pub use output::truncate::*;
#[cfg(not(target_arch = "wasm32"))]
pub use path_utils::{expand_path, resolve_read_path, resolve_to_cwd};
#[cfg(not(target_arch = "wasm32"))]
pub use process::{bash::*, bash_executor::*, exec::*, sleep::*};

pub use tool_definition_wrapper::{
    create_tool_definition_from_agent_tool, wrap_tool_definition, wrap_tool_definitions,
};
pub use types::{
    ExecuteToolDefinition, ToolContextFactory, ToolDefinition, ToolDefinitionRecord, ToolExecution,
    ToolRenderCall, ToolRenderContext, ToolRenderEnvironment, ToolRenderPhase, ToolRenderResult,
    ToolRenderResultCallback, ToolRenderResultOptions, ToolRenderShell, define_tool,
};

#[cfg(not(target_arch = "wasm32"))]
pub use search::tools_manager::*;

pub use output::visual_truncate::*;

/// Tool factories retaining callbacks on the caller's thread.
pub mod local {
    pub use crate::tool_definition_wrapper::local::*;
    #[cfg(not(target_arch = "wasm32"))]
    pub use crate::tools::local::*;
    pub use crate::types::local::*;
}

/// Opt-in helpers for controlled tests.
#[cfg(feature = "test-support")]
pub mod test_support;
