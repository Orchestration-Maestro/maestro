//! Browser attachment components.
/// The session view composing the transcript and the editor.
pub mod agent_interface;
pub mod attachment_tile;
/// Native connection callbacks for retained component roots.
#[cfg(target_arch = "wasm32")]
pub(crate) mod connection;
pub mod diff;
/// Native single-line input.
#[cfg(target_arch = "wasm32")]
pub mod input;
/// Message composer with attachments, reasoning level and model choice.
pub mod message_editor;
/// Completed transcript history placed by emitted position.
pub mod message_list;
/// Open custom-message role callbacks.
pub mod message_renderer_registry;
/// Retained transcript message rows.
pub mod messages;
/// The message being streamed, published once per frame.
pub mod streaming_message_container;
/// Light, dark and system appearance cycling.
pub mod theme_toggle;
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
/// Masked provider key entry tested before saving.
#[cfg(target_arch = "wasm32")]
pub mod provider_key_input;

pub mod expandable_section;
