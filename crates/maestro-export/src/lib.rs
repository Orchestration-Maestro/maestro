//! Serialize session histories to self-contained HTML using supplied viewer assets.
/// Session-document serialization and supplied rendering contracts.
pub mod export_html;
pub use export_html::*;
/// Literal terminal style conversion.
pub mod ansi_to_html;
pub use ansi_to_html::{ansi_lines_to_html, ansi_to_html};
/// Standard custom-tool component rendering.
pub mod tool_renderer;
pub use tool_renderer::{ToolHtmlRendererDeps, create_tool_html_renderer};
