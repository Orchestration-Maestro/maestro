//! Browser attachment components.
pub mod attachment_tile;
/// Retained single-line browser input.
pub mod input;

/// Sandbox runtime messaging.
pub mod sandbox;

pub(crate) mod download_button;
/// Retained sandbox loading and document preparation.
pub mod sandboxed_iframe;

/// Original-text copy control.
pub mod copy_button;

/// Native highlighted code listing.
pub mod code_block;
/// Native Markdown with highlighted code and math.
pub mod markdown_block;

/// Retained preview/source selection.
pub mod preview_code_toggle;
