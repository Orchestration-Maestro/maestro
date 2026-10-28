//! The JSONL protocol server with an optional session runtime: it reads commands from a caller-owned
//! line reader, answers through a caller-owned writer and streams the session events, the
//! extension interface requests and the opted-in workspace events on the same output.
//!
//! The server returns how the connection ended and never exits the process; a native
//! adapter performs the exit.
mod commands;
mod connection;
mod lifecycle;
mod output;
mod ui;

pub use connection::{JsonlLineReader, RpcInput, RpcLineReader, run_rpc_connection};
pub use output::{RpcLine, RpcOutput, RpcRecord, stdout_output};

use crate::rpc_types::{workspace, workspace_schema};
use maestro_export::ExportOperations;
use maestro_process::TerminationSignal;
use maestro_theme::LiveTheme;
use maestro_tools::ToolRenderEnvironment;
use std::{future::Future, io, pin::Pin, rc::Rc};

/// A local operation a caller supplies.
pub type RpcFuture<T> = Pin<Box<dyn Future<Output = T>>>;

/// Writes one workspace event on the connection's output.
pub type WorkspaceEventSink = Rc<dyn Fn(workspace_schema::EventData) -> io::Result<()>>;

/// Runs the workspace commands a connection receives.
pub trait WorkspaceOperations {
    /// Runs `command`, writing the events it opts the connection into through `events`.
    ///
    /// `write_complete` settles once the connection has attempted to write the whole
    /// response, whether or not the write succeeded; it disconnects if the response is
    /// dropped before any attempt. It does not report delivery to the peer.
    fn execute(
        &self,
        command: workspace::RpcCommand,
        events: WorkspaceEventSink,
        write_complete: tokio::sync::oneshot::Receiver<()>,
    ) -> RpcFuture<Result<workspace_schema::ResponseResult, workspace_schema::ErrorBody>>;
}

/// Native process facilities of the server.
pub trait RpcProcessPort {
    /// Delivers each termination signal to `callback`; the returned operation stops the
    /// delivery and makes a later signal take its native default action.
    ///
    /// # Errors
    /// Returns the native registration failure.
    fn register_signals(
        &self,
        callback: Rc<dyn Fn(TerminationSignal)>,
    ) -> io::Result<Box<dyn Fn()>>;
}

/// What a connection works with besides its session runtime.
pub struct RpcModeOptions {
    /// Workspace command operations, absent for a session-only connection.
    pub workspace: Option<Rc<dyn WorkspaceOperations>>,
    /// Session facilities, absent for a workspace-only connection.
    pub session: Option<RpcSessionOptions>,
}

/// The facilities owned by a connection serving a session.
pub struct RpcSessionOptions {
    /// The session runtime.
    pub runtime: Rc<crate::runtime::agent_session_runtime::AgentSessionRuntime>,
    /// The native process facilities.
    pub process: Rc<dyn RpcProcessPort>,
    /// The live theme extensions read.
    pub theme: LiveTheme,
    /// The bundle and theme reads of HTML exports.
    pub export_operations: Rc<dyn ExportOperations>,
    /// The rendering effects of HTML exports.
    pub render_environment: Rc<ToolRenderEnvironment>,
}

/// How a connection ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RpcConnectionOutcome {
    /// The input ended.
    Eof,
    /// An extension requested shutdown.
    Requested,
    /// A termination signal arrived.
    Signal(TerminationSignal),
}

impl RpcConnectionOutcome {
    /// The process exit code: 0, or 128 plus the signal number.
    #[must_use]
    pub fn exit_code(self) -> i32 {
        match self {
            Self::Eof | Self::Requested => 0,
            Self::Signal(signal) => 128 + signal.as_raw(),
        }
    }
}
