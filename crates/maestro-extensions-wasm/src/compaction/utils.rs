//! Native transcript serialization of supplied model messages.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{ExtensionAPI, ExtensionResult, Message};
/// Serializes supplied messages through the existing native conversation owner.
///
/// # Errors
/// Returns the native serializer's cause or an invalid host response.
pub fn serialize_conversation(api: &ExtensionAPI, messages: &[Message]) -> ExtensionResult<String> {
    api.helper("conversation.serialize_conversation", &(messages,))
}
