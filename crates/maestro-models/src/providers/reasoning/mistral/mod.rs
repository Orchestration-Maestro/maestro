#![doc = include_str!("../../../../../../docs/models/reasoning-conversations.md")]

use crate::StreamOptions;
use serde::Serialize;

/// Raw settings for reasoning conversation requests.
#[derive(Clone, Default)]
pub struct MistralOptions {
    /// Shared request settings.
    pub common: StreamOptions,
    /// Requested tool selection.
    pub tool_choice: Option<MistralToolChoice>,
    /// Requested prompt mode.
    pub prompt_mode: Option<MistralPromptMode>,
    /// Requested reasoning effort.
    pub reasoning_effort: Option<MistralReasoningEffort>,
}

/// Tool selection sent with a raw request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MistralToolChoice {
    /// Let the model select tools.
    Auto,
    /// Do not select a tool.
    None,
    /// Select any tool.
    Any,
    /// Require tool selection.
    Required,
    /// Select a named function.
    #[serde(untagged, serialize_with = "serialize_function_choice")]
    Function {
        /// Function name.
        name: String,
    },
}

/// Encode the named function choice in its transport shape.
fn serialize_function_choice<S: serde::Serializer>(
    name: &str,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeStruct;
    /// Selected function identity.
    #[derive(Serialize)]
    struct Function<'a> {
        /// Selected function name.
        name: &'a str,
    }
    let mut choice = serializer.serialize_struct("ToolChoice", 2)?;
    choice.serialize_field("type", "function")?;
    choice.serialize_field("function", &Function { name })?;
    choice.end()
}

/// Prompt mode for raw requests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MistralPromptMode {
    /// Use a reasoning prompt.
    Reasoning,
}

/// Reasoning effort for raw requests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MistralReasoningEffort {
    /// Disable reasoning effort.
    None,
    /// Request high reasoning effort.
    High,
}

#[cfg(test)]
mod tests;

mod wire;

mod request;

mod stream;
pub use stream::{stream_mistral, stream_simple_mistral};

mod chunk;
mod events;
mod sse;

mod content;
