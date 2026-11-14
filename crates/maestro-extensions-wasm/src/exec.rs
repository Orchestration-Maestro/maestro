//! Options and results of explicit command execution.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::AbortSignal;
use serde::Serialize;

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

pub use maestro_interchange::tool::types::ExecResult;
