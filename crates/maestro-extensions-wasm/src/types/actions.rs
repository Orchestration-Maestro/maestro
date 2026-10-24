//! Data carried by ordinary extension actions.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
/// The custom-message fields accepted by an action.
pub type CustomMessageInput = crate::BeforeAgentStartMessage;
pub use maestro_request::messages::{
    MessageDelivery, SendMessageOptions, SendUserMessageOptions, UserMessageDelivery,
};
/// Host-supplied tool metadata with opaque schema text.
pub type ToolInfo =
    maestro_request::tool_info::ToolInfo<maestro_request::parameter_schema::ParameterSchema>;
