//! The JSONL protocol: its session records, the one connection runner, the client that drives
//! a child process, and the native adapters that serve a connection over standard input and
//! output or over the workspace daemon's API socket.
pub mod connection_runner;
#[cfg(target_os = "linux")]
pub mod daemon;
mod rpc_client;
pub mod rpc_types;
pub mod server;

pub use connection_runner::{
    RpcConnectionOutcome, RpcModeOptions, RpcSessionOptions, run_rpc_connection,
};
pub use rpc_client::{ModelInfo, RpcClient, RpcClientError, RpcClientOptions, RpcEventListener};
pub use rpc_types::{
    RpcCommand, RpcExtensionUIRequest, RpcExtensionUIResponse, RpcResponse, RpcSessionState,
};
pub use server::connection::{NativeRpcProcess, run_rpc_mode};
