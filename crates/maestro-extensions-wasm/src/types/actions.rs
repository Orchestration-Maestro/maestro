//! Data carried by ordinary extension actions.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::SourceInfo;
use serde::{Deserialize, Serialize};
/// The custom-message fields accepted by an action.
pub type CustomMessageInput = crate::BeforeAgentStartMessage;
pub use maestro_request::messages::{
    MessageDelivery, SendMessageOptions, SendUserMessageOptions, UserMessageDelivery,
};
/// Host-supplied tool metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    /// Literal tool name.
    pub name: String,
    /// Supplied tool description.
    pub description: String,
    /// Opaque schema text, using the existing data boundary.
    pub parameters: String,
    /// Host-supplied discovery provenance.
    pub source_info: SourceInfo,
}
