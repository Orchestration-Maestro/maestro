//! Browser attachment components.
pub mod attachment_tile;
pub mod changes;
/// Retained single-line browser input.
pub mod fields;
/// Open custom-message role callbacks.
pub mod message_renderer_registry;
/// Retained transcript message rows.
pub mod messages;
pub(crate) mod native_text;
/// Retained reasoning disclosure.
pub mod thinking_block;

/// Sandbox runtime messaging.
pub mod sandbox;

pub mod download_button;
/// Retained sandbox loading and document preparation.
pub mod sandboxed_iframe;

/// Native button element.
#[cfg(target_arch = "wasm32")]
pub mod controls;
/// Original-text copy control.
pub mod copy_button;

/// Native Markdown with highlighted code and math.
pub mod markdown_block;
/// Native highlighted code listing.
pub mod source;

/// Literal native console output with owned copy feedback.
pub mod console_block;
/// Retained preview/source selection.
pub mod preview_code_toggle;

pub mod expandable_section;
