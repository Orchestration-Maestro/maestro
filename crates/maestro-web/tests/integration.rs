//! Shared integration-test executable.
#![cfg(test)]

mod agent_interface_browser;
pub mod artifact_card_support;
mod artifact_elements;
mod artifact_elements_browser;
mod artifact_panel;
mod artifact_panel_browser;
mod artifact_tool_cards;
mod attachment_exports;
mod attachment_overlay;
mod attachment_surfaces;
mod attachment_tiles;
mod browser_effects;
mod browser_errors;
mod browser_fixture_support;
mod browser_fixtures;
mod browser_formatting;
mod chat_example_browser;
mod chat_panel_browser;
mod code_blocks;
mod contributions;
mod contributions_browser;
mod copy_buttons;
mod custom_provider_card_browser;
mod dialogs_browser;
mod diff_factory;
mod document_card_support;
mod document_tool_cards;
mod document_tool_cards_browser;
mod docx_preview_browser;
mod dom_content_browser;
mod excel_views;
mod excel_views_browser;
mod expandable_output_browser;
mod export_markdown;
mod icon_rendering;
mod localization;
mod markdown_blocks;
mod markdown_blocks_browser;
mod markdown_svg_artifacts;
mod markdown_svg_artifacts_browser;
mod message_editor_browser;
mod message_errors;
mod message_renderers;
mod message_renderers_browser;
mod message_tool_arguments;
mod native_artifact_console_browser;
mod native_artifact_console_support;
mod native_artifact_controls_browser;
mod native_attachment_browser;
mod native_console_browser;
mod native_document_artifacts_browser;
mod native_html_artifact_browser;
mod native_html_artifact_support;
mod native_markdown_browser;
mod native_messages_browser;
mod native_simple_artifacts_browser;
mod native_thinking_browser;
mod pdf_views;
mod pdf_views_browser;
mod prompt_exports;
mod provider_dialogs_browser;
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
mod settings_dialogs_browser;
mod storage_exports;
mod text_image_artifacts;
mod text_image_artifacts_browser;
mod theme_toggle_browser;
mod tool_cards;
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
pub mod contributions_support;
pub mod copy_button_support;
pub mod dialog_support;
pub mod effects_support;
#[cfg(target_arch = "wasm32")]
#[path = "export_support/browser.rs"]
mod export_mount;
mod export_support;
#[path = "highlighter_support/failure.rs"]
pub mod failing_highlighter;
pub mod highlighter_support;
pub mod input_support;
pub mod markdown_support;
pub mod markdown_svg_support;
#[cfg(target_arch = "wasm32")]
pub mod native_simple_artifacts_support;
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
#[cfg(target_arch = "wasm32")]
pub mod tool_cards_support;
pub mod tool_renderer_support;
use tool_renderer_support as recipes;

mod message_renderer_support;
use tool_renderer_support as renderer_recipes;

mod offline_tool_card_support;
mod offline_tool_cards;

mod offline_tool_cards_browser;

#[cfg(target_arch = "wasm32")]
#[path = "../../maestro-app/tests/support/artifact_presentation.rs"]
mod artifact_presentation_support;
mod sample_sessions_browser;

mod native_tool_browser;
mod native_transcript_browser;

mod native_remaining_artifacts_browser;
