//! The JSONL protocol records, server and client at the application root.

pub use crate::rpc_transport::{
    ModelInfo, RpcClient, RpcClientError, RpcClientOptions, RpcEventListener,
};

pub use crate::connection_runner::{RpcConnectionOutcome, RpcModeOptions, run_rpc_connection};
pub use crate::rpc_types::{
    RpcCommand, RpcExtensionUIRequest, RpcExtensionUIResponse, RpcResponse, RpcSessionState,
};
