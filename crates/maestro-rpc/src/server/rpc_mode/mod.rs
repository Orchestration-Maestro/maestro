//! Serves one session runtime over standard input and output, then exits the process.
mod native;

pub use native::NativeRpcProcess;

use maestro_app::output_guard::take_over_stdout;
use maestro_app::rpc_mode::{JsonlLineReader, stdout_output};
use maestro_app::{AgentSessionRuntime, RpcModeOptions, run_rpc_connection};
use std::{convert::Infallible, io, rc::Rc};

/// Serves `runtime_host` over standard input and output until the input ends, an extension
/// requests shutdown or a termination signal arrives, then exits the process with the
/// connection's exit code: 0, or 128 plus the signal's number.
///
/// Standard output carries protocol records only; other managed output goes to standard
/// error. Must run inside a [`tokio::task::LocalSet`] on a runtime with its signal driver.
///
/// # Errors
/// Returns an input or output failure, or the failure of binding or disposing the session.
pub async fn run_rpc_mode(
    runtime_host: Rc<AgentSessionRuntime>,
    options: RpcModeOptions,
) -> io::Result<Infallible> {
    take_over_stdout();
    let input = Box::new(JsonlLineReader::new(tokio::io::stdin()));
    let outcome = run_rpc_connection(runtime_host, input, stdout_output(), options).await?;
    std::process::exit(outcome.exit_code())
}
