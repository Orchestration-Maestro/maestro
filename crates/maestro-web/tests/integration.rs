//! Shared integration-test executable.
#![cfg(test)]

mod artifact_elements;
mod artifact_elements_browser;
mod artifact_panel;
mod artifact_panel_browser;
mod attachment_exports;
mod attachment_overlay;
mod attachment_surfaces;
mod attachment_tiles;
mod browser_effects;
mod browser_errors;
mod browser_formatting;
mod code_blocks;
mod copy_buttons;
mod dialog_shell;
mod dialog_shell_browser;
mod excel_views;
mod excel_views_browser;
mod icon_rendering;
mod input_native;
mod input_widget;
mod localization;
mod markdown_blocks;
mod markdown_blocks_browser;
mod markdown_svg_artifacts;
mod markdown_svg_artifacts_browser;
mod pdf_views;
mod pdf_views_browser;
mod provider_exports;
mod runtime_bridge;
mod sandbox_execution_browser;
mod sandbox_exports;
mod sandbox_loading_browser;
mod session_documents;
mod session_download_browser;
mod session_downloads;
mod session_header;
mod session_header_browser;
mod session_link_composition;
mod session_link_transitions;
mod session_links;
mod session_links_browser;
mod storage_exports;
mod text_image_artifacts;
mod text_image_artifacts_browser;
mod tool_renderers;
mod tool_renderers_browser;

#[cfg(target_arch = "wasm32")]
pub(crate) use artifact_support::presentation;
pub(crate) use attachment_support::canvas_paint;
#[path = "support/brand.rs"]
pub mod brand;
#[cfg(target_arch = "wasm32")]
pub(crate) use sandbox_loading_support::placement;
pub mod artifact_support;
pub mod attachment_support;
pub mod copy_button_support;
pub mod effects_support;
pub mod highlighter_support;
pub mod input_support;
pub mod markdown_support;
pub mod markdown_svg_support;
pub mod panel_browser_support;
pub mod pdf_support;
pub mod reading_browser_support;
pub mod reading_slots_support;
#[cfg(target_arch = "wasm32")]
pub mod sandbox_execution_support;
#[cfg(target_arch = "wasm32")]
pub mod sandbox_loading_support;
mod session_document_support;
mod session_download_support;
pub mod session_header_support;
pub mod session_links_support;
pub mod support;
#[path = "artifact_support/text_image.rs"]
pub mod text_image_support;
pub mod tool_renderer_support;
