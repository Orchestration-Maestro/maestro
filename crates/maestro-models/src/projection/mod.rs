/// Conversation adaptation and tool-result association.
mod transform_messages;
pub use transform_messages::transform_messages;
pub(crate) use transform_messages::{
    IMAGE_PLACEHOLDER, normalize_tool_call_id, tool_call_id_characters,
};
