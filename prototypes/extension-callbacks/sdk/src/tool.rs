//! Tool definitions and the JSON view of tool results.

use std::rc::Rc;

use serde_json::Value;

use crate::api::{ExtensionFuture, ExtensionResult};
use crate::bindings::maestro::extension::types::{ToolMetadata, ToolResult};
use crate::context::{AbortSignal, ExtensionContext};

/// Result of a tool call with details as JSON.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentToolResult {
    /// Text blocks returned to the model.
    pub content: Vec<String>,
    /// Structured details; absent stays distinct from JSON null.
    pub details: Option<Value>,
    /// Whether the run ends after this result.
    pub terminate: Option<bool>,
}

impl From<AgentToolResult> for ToolResult {
    fn from(result: AgentToolResult) -> Self {
        Self {
            content: result.content,
            details: result.details.map(|details| details.to_string()),
            terminate: result.terminate,
        }
    }
}

impl TryFrom<ToolResult> for AgentToolResult {
    type Error = String;

    fn try_from(result: ToolResult) -> Result<Self, String> {
        let details = result
            .details
            .map(|text| serde_json::from_str::<Value>(&text).map_err(|error| error.to_string()))
            .transpose()?;
        Ok(Self {
            content: result.content,
            details,
            terminate: result.terminate,
        })
    }
}

/// Sends a partial result while a tool runs.
pub type AgentToolUpdateCallback = Rc<dyn Fn(AgentToolResult) -> ExtensionResult<()>>;

/// Rewrites raw arguments before the tool runs.
pub type PrepareArguments = Rc<dyn Fn(Value) -> ExtensionResult<Value>>;

/// Runs a tool: call id, prepared arguments, signal, progress callback, context.
pub type ToolExecute = Rc<
    dyn Fn(
        String,
        Value,
        Option<AbortSignal>,
        Option<AgentToolUpdateCallback>,
        ExtensionContext,
    ) -> ExtensionFuture<'static, AgentToolResult>,
>;

/// A tool: generated metadata plus its callbacks.
#[derive(Clone)]
pub struct ToolDefinition {
    /// Registration data shared with the host.
    pub metadata: ToolMetadata,
    /// Optional argument preparation.
    pub prepare_arguments: Option<PrepareArguments>,
    /// The tool body.
    pub execute: ToolExecute,
}

impl ToolDefinition {
    /// Builds a tool without argument preparation.
    pub fn new<F>(metadata: ToolMetadata, execute: F) -> Self
    where
        F: Fn(
                String,
                Value,
                Option<AbortSignal>,
                Option<AgentToolUpdateCallback>,
                ExtensionContext,
            ) -> ExtensionFuture<'static, AgentToolResult>
            + 'static,
    {
        Self {
            metadata,
            prepare_arguments: None,
            execute: Rc::new(execute),
        }
    }

    /// Adds argument preparation.
    #[must_use]
    pub fn with_prepare_arguments(
        mut self,
        prepare: impl Fn(Value) -> ExtensionResult<Value> + 'static,
    ) -> Self {
        self.prepare_arguments = Some(Rc::new(prepare));
        self
    }
}
