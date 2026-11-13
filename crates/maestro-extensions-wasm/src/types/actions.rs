//! Data carried by ordinary extension actions.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
/// The custom-message fields accepted by an action.
pub type CustomMessageInput = crate::BeforeAgentStartMessage;
pub use maestro_interchange::messages::{
    MessageDelivery, SendMessageOptions, SendUserMessageOptions, UserMessageDelivery,
};
/// Host-supplied tool metadata with the shared parameter schema.
pub type ToolInfo = maestro_interchange::tool_info::ToolInfo<
    maestro_interchange::parameter_schema::ParameterSchema<crate::schema_code::SchemaCode>,
>;
