//! Returned shell operations retain their authored identity and the native borrowed sink.
use super::{Call, Handle};
use crate::lifecycle::replies::Reply;
use crate::local_resources::{Registry, SignalEndpoint, bash_output};
use maestro_extensions::{BashError, BashExecOptions, BashExecResult, BashOperations, Local};
use wasmtime::component::{ComponentType, Lift, Lower};

/// Actual Local-family borrowed output operand.
type LocalOptions<'a> = BashExecOptions<'a, dyn FnMut(&[u8]) + 'a>;

/// Literal canonical execution operands, with no process policy added at the boundary.
#[derive(ComponentType, Lift, Lower, Clone)]
#[component(record)]
pub(crate) struct BashRequest {
    /// Exact authored shell command.
    pub(crate) command: String,
    /// Exact caller-selected directory.
    pub(crate) cwd: String,
    /// Optional caller deadline in seconds.
    pub(crate) timeout: Option<f64>,
    /// Complete optional environment, including unset entries.
    pub(crate) env: Option<Vec<(String, Option<String>)>>,
}

/// One real guest execution consumed by the original Store driver.
pub(crate) struct BashCall<H = Handle> {
    /// Retains the authored operations until execution settles, independently of aliases.
    pub(crate) owner: H,
    /// Unmodified owned operands.
    pub(crate) request: BashRequest,
    /// Per-execution original output frame.
    pub(crate) output: bash_output::Endpoint,
    /// Independently optional original cancellation.
    pub(crate) signal: Option<SignalEndpoint>,
    /// Exact guest numeric status or error.
    pub(crate) reply: Reply<Option<f64>>,
}

/// Local native adapter; the original registry owns each output frame independently.
pub(crate) struct ReturnedBash {
    /// Retained identity of the authored operations and its Store lifecycle.
    owner: Handle,
    /// Original single native request pump, never a receiver copy.
    registry: Registry,
}

impl ReturnedBash {
    /// Retains returned operations independently of event editing or decision encoding.
    pub(crate) fn new(owner: Handle, registry: Registry) -> Self {
        Self { owner, registry }
    }
}

impl BashOperations<Local> for ReturnedBash {
    fn exec<'a>(
        &'a self,
        command: &'a str,
        cwd: &'a str,
        options: LocalOptions<'a>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<BashExecResult, BashError>> + 'a>>
    {
        Box::pin(async move {
            self.execute(command, cwd, options)
                .await
                .map(|exit_code| BashExecResult { exit_code })
                .map_err(|error| BashError::Other(error.into()))
        })
    }
}

impl ReturnedBash {
    /// Drains each frame before exposing the actual operation result to its native caller.
    async fn execute(
        &self,
        command: &str,
        cwd: &str,
        options: LocalOptions<'_>,
    ) -> Result<Option<f64>, String> {
        let (output, mut consumer) = self.registry.bash_output()?;
        let signal = options
            .signal
            .map(|signal| self.registry.signal(signal.clone()))
            .transpose()?;
        let request = BashRequest {
            command: command.to_owned(),
            cwd: cwd.to_owned(),
            timeout: options.timeout,
            env: options
                .env
                .map(|env| {
                    env.iter()
                        .map(environment_entry)
                        .collect::<Result<Vec<_>, String>>()
                })
                .transpose()?,
        };
        let operation = self.owner.endpoint().call(&self.registry, |reply| {
            Call::Bash(BashCall {
                owner: self.owner.clone(),
                request,
                output,
                signal,
                reply,
            })
        });
        consumer.finish(operation, options.on_data).await
    }
}

/// Projects a native environment entry without losing unset values or accepting lossy text.
pub(crate) fn environment_entry(
    (key, value): (&std::ffi::OsString, &Option<std::ffi::OsString>),
) -> Result<(String, Option<String>), String> {
    let key = key
        .to_str()
        .ok_or("extension environment key is not UTF-8")?;
    let value = value
        .as_ref()
        .map(|value| {
            value
                .to_str()
                .ok_or("extension environment value is not UTF-8")
                .map(str::to_owned)
        })
        .transpose()?;
    Ok((key.to_owned(), value))
}
