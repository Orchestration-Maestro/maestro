//! Browser attachment components.
pub mod attachment_tile;
pub mod diff;
/// Retained single-line browser input.
pub mod input;
/// Completed transcript history placed by emitted position.
pub mod message_list;
/// Open custom-message role callbacks.
pub mod message_renderer_registry;
/// Retained transcript message rows.
pub mod messages;
/// Retained reasoning disclosure.
pub mod thinking_block;

/// Sandbox runtime messaging.
pub mod sandbox;

pub mod download_button;
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
/// Native custom-provider summary and current actions.
pub mod custom_provider_card;
/// Shared native mode selection.
#[cfg(target_arch = "wasm32")]
pub(crate) mod mode_toggle;
/// Retained preview/source selection.
pub mod preview_code_toggle;

pub mod expandable_section;
