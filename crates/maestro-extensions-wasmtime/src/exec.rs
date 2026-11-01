//! Plain command operands forwarded to the existing native execution owner.
use crate::local_resources::{SignalEndpoint, Work};
use maestro_extensions::{Cancellation, ExtensionAPI, Local};
use serde::Deserialize;
use std::time::Duration;

/// Owned wire data with its independently retained cancellation resource.
pub(crate) struct Request {
    /// Exact ordinary operand encoding.
    pub(crate) data: String,
    /// Original native signal, never an identity hidden in JSON.
    pub(crate) signal: Option<SignalEndpoint>,
}

/// Ordinary options; cancellation crosses as a separate borrowed capability.
#[derive(Default, Deserialize)]
struct Options {
    /// Optional millisecond deadline.
    timeout: Option<f64>,
    /// Literal directory; absence selects the facade's captured directory.
    cwd: Option<String>,
}

/// Starts one native execution with owned operands and settles its original caller.
pub(crate) fn start(
    data: String,
    api: ExtensionAPI<Local>,
    signal: Option<Cancellation>,
    reply: tokio::sync::oneshot::Sender<Result<String, String>>,
) -> Work {
    Box::pin(async move {
        let result = async {
            let (command, args, options): (String, Vec<String>, Option<Options>) =
                crate::writer_reads::decode(&data)?;
            let options = options.unwrap_or_default();
            let exec = maestro_extensions::ExecOptions {
                signal: signal.as_ref(),
                timeout: options
                    .timeout
                    .filter(|value| *value > 0.0)
                    .map(milliseconds),
            };
            let result = api
                .exec(&command, &args, options.cwd.as_deref(), exec)
                .map_err(|error| error.to_string())?
                .await
                .map_err(|error| error.to_string())?;
            let result = serde_json::json!({"stdout": result.stdout, "stderr": result.stderr,
                "code": result.code, "killed": result.killed});
            Ok(result.to_string())
        }
        .await;
        let _ = reply.send(result);
    })
}

/// A positive millisecond count as a native deadline: past the range it saturates to no
/// reachable deadline, and below one nanosecond it stays a deadline of one nanosecond.
fn milliseconds(value: f64) -> Duration {
    Duration::try_from_secs_f64(value / 1000.0).map_or(Duration::MAX, |timeout| {
        timeout.max(Duration::from_nanos(1))
    })
}
