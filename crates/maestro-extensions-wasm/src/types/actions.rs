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
/// Custom-message delivery placement.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MessageDelivery {
    /// Request steering delivery.
    Steer,
    /// Request follow-up delivery.
    FollowUp,
    /// Request delivery at the next turn.
    NextTurn,
}
/// User-message delivery placement.
///
/// User messages have no next-turn delivery variant:
/// ```compile_fail,E0599
/// let delivery = maestro_extensions_wasm::UserMessageDelivery::NextTurn;
/// ```
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UserMessageDelivery {
    /// Request steering delivery.
    Steer,
    /// Request follow-up delivery.
    FollowUp,
}
/// Custom-message delivery options; no local defaults are applied.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Whether the host should trigger a turn.
    pub trigger_turn: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional requested delivery placement.
    pub deliver_as: Option<MessageDelivery>,
}
/// User-message delivery options.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendUserMessageOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional requested delivery placement.
    pub deliver_as: Option<UserMessageDelivery>,
}
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
