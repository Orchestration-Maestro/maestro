//! Tool output selection and accounting.

#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod render_parts;
pub mod render_utils;
pub mod truncate;

#[cfg(not(target_arch = "wasm32"))]
pub mod output_accumulator;

/// Wrap text in a result block.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn text_block(text: String) -> maestro_models::UserBlock {
    maestro_models::UserBlock::Text(maestro_models::TextContent {
        text,
        text_signature: None,
    })
}

/// Wrap text in a result with optional details and no termination request.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn text_result<D>(
    text: String,
    details: Option<D>,
) -> maestro_agent::AgentToolResult<D> {
    maestro_agent::AgentToolResult {
        content: vec![text_block(text)],
        details,
        terminate: None,
    }
}

pub mod visual_truncate;
