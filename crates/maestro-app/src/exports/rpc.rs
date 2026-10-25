//! The JSONL protocol records, server and client at the application root.

pub use crate::rpc_client::{
    ModelInfo, RpcClient, RpcClientError, RpcClientOptions, RpcEventListener,
};

pub use crate::rpc_mode::{
    RpcConnectionOutcome, RpcModeOptions, RpcSessionOptions, run_rpc_connection,
};
pub use crate::rpc_types::{
    RpcCommand, RpcExtensionUIRequest, RpcExtensionUIResponse, RpcResponse, RpcSessionState,
};
