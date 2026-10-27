//! Options and results of explicit command execution.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::AbortSignal;
use serde::{Deserialize, Serialize};

/// Options of one explicit execution; absent fields take the host's defaults.
#[derive(Clone, Default, Serialize)]
pub struct ExecOptions {
    /// Terminates the command when aborted; passed as a capability, never as data.
    #[serde(skip)]
    pub signal: Option<AbortSignal>,
    /// Milliseconds before termination; zero or a negative value sets no deadline.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<f64>,
    /// Literal working directory; absent uses the extension's directory and empty
    /// inherits the host process directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

/// What one explicit execution produced.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ExecResult {
    /// Captured standard output.
    pub stdout: String,
    /// Captured standard error.
    pub stderr: String,
    /// Exit code; 0 when a signal ended the command, 1 when it did not start.
    pub code: i32,
    /// Whether the abort or the timeout requested termination.
    pub killed: bool,
}
