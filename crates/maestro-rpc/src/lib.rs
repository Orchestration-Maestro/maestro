//! The JSONL protocol over standard input and output: a native adapter that serves one
//! session runtime through the application's protocol server and exits with its outcome.
pub mod server;

pub use server::rpc_mode::{NativeRpcProcess, run_rpc_mode};
