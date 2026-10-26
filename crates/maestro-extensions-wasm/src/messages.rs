//! Application-owned conversation messages.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
pub use maestro_request::messages::{
    BashExecutionMessage, BranchSummaryMessage, CompactionSummaryMessage,
};
/// An extension-injected conversation message with opaque details text.
pub type CustomMessage = maestro_request::messages::CustomMessage<crate::JsonText>;

/// Projects supplied application messages through the native conversation owner.
///
/// # Errors
/// Returns the host's failure or an invalid message response.
pub fn convert_to_llm(
    api: &crate::ExtensionAPI,
    messages: &[crate::AgentMessage],
) -> crate::ExtensionResult<Vec<crate::Message>> {
    api.helper("conversation.convert_to_llm", &(messages,))
}
