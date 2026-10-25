//! The client's options, its event callback and its failures.
use maestro_models::Model;
use serde_json::Value;
use std::{collections::BTreeMap, fmt, rc::Rc};

/// How a client launches its child.
#[derive(Default)]
pub struct RpcClientOptions {
    /// The executable; `dist/maestro` with the platform's executable suffix when absent. A
    /// relative path is resolved against the child's working directory.
    pub cli_path: Option<String>,
    /// The child's working directory; the current one when absent.
    pub cwd: Option<String>,
    /// Variables set for the child on top of the inherited environment.
    pub env: BTreeMap<String, String>,
    /// The provider to select; an empty one selects none.
    pub provider: Option<String>,
    /// The model to select; an empty one selects none.
    pub model: Option<String>,
    /// Arguments appended after the mode and selection flags.
    pub args: Vec<String>,
}

/// A model the session can select.
pub type ModelInfo = Model;

/// Receives each parsed record the child writes that answers no pending request. A failure
/// stops the delivery of that record to the callbacks after this one.
pub type RpcEventListener = Rc<dyn Fn(&Value) -> Result<(), RpcClientError>>;

/// Why a client operation failed.
#[derive(Debug)]
pub enum RpcClientError {
    /// The child is already running.
    AlreadyStarted,
    /// No child is running.
    NotStarted,
    /// The child exited while starting.
    ProcessExit {
        /// Its exit code; absent when a signal ended it.
        code: Option<i32>,
        /// The diagnostic text it had written.
        stderr: String,
    },
    /// The child answered the request with a failure.
    Response(String),
    /// No answer arrived in time.
    RequestTimeout {
        /// The command type.
        command: String,
        /// The diagnostic text written so far.
        stderr: String,
    },
    /// The session did not finish its run in time.
    IdleTimeout {
        /// The diagnostic text written so far.
        stderr: String,
    },
    /// The session did not finish its run in time while events were collected.
    CollectionTimeout {
        /// The diagnostic text written so far.
        stderr: String,
    },
    /// The client stopped, or its child ended, before the operation finished.
    Stopped,
    /// A native process or pipe failure.
    Io(std::io::Error),
    /// An answer that does not have its command's shape.
    Decode(serde_json::Error),
}

impl fmt::Display for RpcClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyStarted => formatter.write_str("Client already started"),
            Self::NotStarted => formatter.write_str("Client not started"),
            Self::ProcessExit { code, stderr } => {
                let code = code.map_or_else(|| "null".to_owned(), |code| code.to_string());
                write!(
                    formatter,
                    "Agent process exited immediately with code {code}. Stderr: {stderr}"
                )
            }
            Self::Response(error) => formatter.write_str(error),
            Self::RequestTimeout { command, stderr } => write!(
                formatter,
                "Timeout waiting for response to {command}. Stderr: {stderr}"
            ),
            Self::IdleTimeout { stderr } => write!(
                formatter,
                "Timeout waiting for agent to become idle. Stderr: {stderr}"
            ),
            Self::CollectionTimeout { stderr } => {
                write!(formatter, "Timeout collecting events. Stderr: {stderr}")
            }
            Self::Stopped => formatter.write_str("Client stopped"),
            Self::Io(error) => error.fmt(formatter),
            Self::Decode(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for RpcClientError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Decode(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for RpcClientError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for RpcClientError {
    fn from(error: serde_json::Error) -> Self {
        Self::Decode(error)
    }
}
