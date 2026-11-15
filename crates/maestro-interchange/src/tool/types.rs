//! Tool record vocabulary.
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

/// Tool scheduling preference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolExecutionMode {
    /// Non-overlapping execution.
    Sequential,
    /// Concurrent execution.
    Parallel,
}

use crate::{
    diagnostics::DiagnosticErrorInfo,
    presence::Omittable,
    types::{TextContent, UserBlock},
};

/// Tool output with caller-selected optional field representations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentToolResult<D: Omittable, T: Omittable> {
    /// Ordered text and image output.
    pub content: Vec<UserBlock>,
    /// Caller-defined result data.
    #[serde(default, skip_serializing_if = "Omittable::is_omitted")]
    pub details: D,
    /// Requested termination.
    #[serde(default, skip_serializing_if = "Omittable::is_omitted")]
    pub terminate: T,
}

impl<D> AgentToolResult<Option<D>, Option<bool>> {
    /// Wrap text in a result with optional details and no termination request.
    #[must_use]
    pub fn text_result(text: String, details: Option<D>) -> Self {
        Self {
            content: vec![UserBlock::Text(TextContent::new(text))],
            details,
            terminate: None,
        }
    }
}

/// The exit result of a shell execution, without narrowing a supplied implementation's value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BashExecResult {
    /// Numeric status, or no status when a signal ended the process.
    pub exit_code: Option<f64>,
}

/// What one explicit command execution produced.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ExecResult {
    /// The captured standard output, decoded lossily.
    pub stdout: String,
    /// The captured standard error, decoded lossily.
    pub stderr: String,
    /// The exit code; 0 when a signal ended the command, 1 when it did not start or
    /// waiting failed.
    pub code: i32,
    /// Whether the abort or the timeout requested termination.
    pub killed: bool,
}

/// Glob constraints forwarded to supplied find operations.
pub struct FindGlobOptions {
    /// Ignore patterns, in their supplied order.
    pub ignore: Vec<String>,
    /// The effective result limit, without narrowing the authored number.
    pub limit: f64,
}

/// The built-in tool identities; custom tool names remain ordinary strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolName {
    /// The read tool.
    Read,
    /// The bash tool.
    Bash,
    /// The edit tool.
    Edit,
    /// The write tool.
    Write,
    /// The grep tool.
    Grep,
    /// The find tool.
    Find,
    /// The ls tool.
    Ls,
}

impl FromStr for ToolName {
    type Err = DiagnosticErrorInfo;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name {
            "read" => Ok(Self::Read),
            "bash" => Ok(Self::Bash),
            "edit" => Ok(Self::Edit),
            "write" => Ok(Self::Write),
            "grep" => Ok(Self::Grep),
            "find" => Ok(Self::Find),
            "ls" => Ok(Self::Ls),
            _ => Err(DiagnosticErrorInfo::error(format!(
                "Unknown tool name: {name}"
            ))),
        }
    }
}

impl fmt::Display for ToolName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Read => "read",
            Self::Bash => "bash",
            Self::Edit => "edit",
            Self::Write => "write",
            Self::Grep => "grep",
            Self::Find => "find",
            Self::Ls => "ls",
        })
    }
}

/// Execution progress of a tool row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ToolRenderPhase {
    /// Whether execution has started.
    pub execution_started: bool,
    /// Whether the arguments are complete.
    pub args_complete: bool,
    /// Whether the result is still streaming.
    pub is_partial: bool,
}

/// Flags that change how a result is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolRenderResultOptions {
    /// Whether the result is shown in its expanded form.
    pub expanded: bool,
    /// Whether the result is still streaming.
    pub is_partial: bool,
}

/// A line's text and its independent truncation flag.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TruncateLineResult {
    /// The original text, or the text clipped on a scalar boundary.
    pub text: String,
    /// Whether the original line was shortened.
    pub was_truncated: bool,
}

/// Independent tool-output limits; absent fields select their defaults.
#[derive(Clone, Copy, Debug, Default)]
pub struct TruncationOptions {
    /// Maximum retained entries; `usize::MAX` leaves this budget unbounded.
    pub max_lines: Option<usize>,
    /// Maximum retained UTF-8 bytes; `usize::MAX` leaves this budget unbounded.
    pub max_bytes: Option<usize>,
}
