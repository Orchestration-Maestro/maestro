//! The JSONL protocol records and server at the application root.

pub use crate::rpc_mode::{RpcConnectionOutcome, RpcModeOptions, run_rpc_connection};
pub use crate::rpc_types::{
    RpcCommand, RpcExtensionUIRequest, RpcExtensionUIResponse, RpcResponse, RpcSessionState,
};
