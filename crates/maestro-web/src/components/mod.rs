//! Browser attachment components.
pub mod attachment_tile;
pub mod diff;
/// Retained single-line browser input.
pub mod input;
/// Open custom-message role callbacks.
pub mod message_renderer_registry;
/// Retained transcript message rows.
pub mod messages;
pub(crate) mod native_text;
/// Retained reasoning disclosure.
pub mod thinking_block;

/// Sandbox runtime messaging.
pub mod sandbox;

pub(crate) mod download_button;
/// Retained sandbox loading and document preparation.
pub mod sandboxed_iframe;

/// Native button element.
#[cfg(target_arch = "wasm32")]
pub mod button;
/// Original-text copy control.
pub mod copy_button;

/// Native highlighted code listing.
pub mod code_block;
/// Native Markdown with highlighted code and math.
pub mod markdown_block;

/// Literal native console output with owned copy feedback.
pub mod console_block;
/// Retained preview/source selection.
pub mod preview_code_toggle;

pub mod expandable_section;
