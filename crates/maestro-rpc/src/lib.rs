//! The JSONL protocol over standard input and output, and over the workspace daemon's API
//! socket: native adapters that serve the application's protocol server.
#[cfg(target_os = "linux")]
pub mod daemon;
pub mod server;

pub use server::rpc_mode::{NativeRpcProcess, run_rpc_mode};
