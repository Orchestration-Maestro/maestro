//! Guest aliases of the shared extension schemas.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

/// Shared schema specialized for the guest operands.
pub type ToolExecutionStartEvent =
    maestro_request::extension_events::tool_events::ToolExecutionStartEvent<crate::JsonText>;
/// Shared schema specialized for the guest operands.
pub type ToolExecutionUpdateEvent =
    maestro_request::extension_events::tool_events::ToolExecutionUpdateEvent<
        crate::JsonText,
        crate::JsonText,
    >;
/// Shared schema specialized for the guest operands.
pub type ToolExecutionEndEvent =
    maestro_request::extension_events::tool_events::ToolExecutionEndEvent<crate::JsonText>;
/// Shared schema specialized for the guest operands.
pub type ToolCallEvent<I = ToolInput> =
    maestro_request::extension_events::tool_events::ToolCallEvent<I>;
/// Shared schema specialized for the guest operands.
pub type ToolResultEvent<D = ToolDetails> =
    maestro_request::extension_events::tool_events::ToolResultEvent<D, crate::JsonText>;
/// Shared schema specialized for the guest operands.
pub type CustomToolCallEvent =
    maestro_request::extension_events::tool_events::CustomToolCallEvent<crate::JsonText>;
/// Shared schema specialized for the guest operands.
pub type CustomToolResultEvent =
    maestro_request::extension_events::tool_events::CustomToolResultEvent<
        crate::JsonText,
        crate::JsonText,
    >;
/// Shared schema specialized for the guest operands.
pub type BashToolCallEvent =
    maestro_request::extension_events::tool_events::ToolCallEvent<crate::BashToolInput>;
/// Shared schema specialized for the guest operands.
pub type BashToolResultEvent = maestro_request::extension_events::tool_events::ToolResultEvent<
    crate::BashToolDetails,
    crate::JsonText,
>;
/// Shared schema specialized for the guest operands.
pub type ReadToolCallEvent =
    maestro_request::extension_events::tool_events::ToolCallEvent<crate::ReadToolInput>;
/// Shared schema specialized for the guest operands.
pub type ReadToolResultEvent = maestro_request::extension_events::tool_events::ToolResultEvent<
    crate::ReadToolDetails,
    crate::JsonText,
>;
/// Shared schema specialized for the guest operands.
pub type EditToolCallEvent =
    maestro_request::extension_events::tool_events::ToolCallEvent<crate::EditToolInput>;
/// Shared schema specialized for the guest operands.
pub type EditToolResultEvent = maestro_request::extension_events::tool_events::ToolResultEvent<
    crate::EditToolDetails,
    crate::JsonText,
>;
/// Shared schema specialized for the guest operands.
pub type WriteToolCallEvent =
    maestro_request::extension_events::tool_events::ToolCallEvent<crate::WriteToolInput>;
/// Shared schema specialized for the guest operands.
pub type WriteToolResultEvent =
    maestro_request::extension_events::tool_events::ToolResultEvent<(), crate::JsonText>;
/// Shared schema specialized for the guest operands.
pub type GrepToolCallEvent =
    maestro_request::extension_events::tool_events::ToolCallEvent<crate::GrepToolInput>;
/// Shared schema specialized for the guest operands.
pub type GrepToolResultEvent = maestro_request::extension_events::tool_events::ToolResultEvent<
    crate::GrepToolDetails,
    crate::JsonText,
>;
/// Shared schema specialized for the guest operands.
pub type FindToolCallEvent =
    maestro_request::extension_events::tool_events::ToolCallEvent<crate::FindToolInput>;
/// Shared schema specialized for the guest operands.
pub type FindToolResultEvent = maestro_request::extension_events::tool_events::ToolResultEvent<
    crate::FindToolDetails,
    crate::JsonText,
>;
/// Shared schema specialized for the guest operands.
pub type LsToolCallEvent =
    maestro_request::extension_events::tool_events::ToolCallEvent<crate::LsToolInput>;
/// Shared schema specialized for the guest operands.
pub type LsToolResultEvent = maestro_request::extension_events::tool_events::ToolResultEvent<
    crate::LsToolDetails,
    crate::JsonText,
>;
use serde::{Deserialize, Serialize};
/// JSON retained independently of the tool's registered name.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ToolInput(Box<serde_json::value::RawValue>);

impl ToolInput {
    /// Decodes an authored view without changing the retained JSON.
    ///
    /// # Errors
    /// Returns the selected type's JSON decoding error.
    pub fn decode<'de, T: Deserialize<'de>>(&'de self) -> Result<T, serde_json::Error> {
        serde_json::from_str(self.0.get())
    }

    /// Encodes an authored value, rejecting nonfinite numbers before encoding.
    ///
    /// # Errors
    /// Returns an error for nonfinite numbers or failed serialization.
    pub fn from_value<T: Serialize>(value: &T) -> Result<Self, String> {
        let text = crate::types::finite::encode_value(value)?;
        serde_json::value::RawValue::from_string(text)
            .map(Self)
            .map_err(|error| error.to_string())
    }
}

/// Result details use the same JSON carrier as tool input.
pub type ToolDetails = ToolInput;

pub use maestro_request::extension_events::tool_events::{
    is_bash_tool_result, is_edit_tool_result, is_find_tool_result, is_grep_tool_result,
    is_ls_tool_result, is_read_tool_result, is_tool_call_event_type, is_write_tool_result,
};
