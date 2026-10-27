//! Original native local operations on the sole request pump, without process policy here.
use super::{Admission, Capability, Lease, Message, Registry, builtin::Operation, pump::State};
use crate::{
    api_reads,
    guest_calls::{Handle, SynchronousKind, bash::BashRequest},
};
use maestro_extensions::{BashExecOptions, BashOperations, Cancellation, EventBus, Local};
use std::{rc::Rc, sync::Arc};

/// Independently retained native shell identity, safe to hold on the Store thread.
#[derive(Clone)]
pub(crate) struct Endpoint(Arc<Lease>);

/// An execution's original operands and independently scoped observers.
pub(crate) struct Execution {
    /// Literal shell operands, including optional seconds and complete environment.
    pub(crate) request: BashRequest,
    /// Original caller signal, absent when not supplied.
    pub(crate) signal: Option<super::SignalEndpoint>,
    /// Scoped raw-byte observer in the existing callback table.
    pub(crate) output: Handle,
    /// Original native facade supplies its callback frame and diagnostic destination.
    pub(crate) api: api_reads::Endpoint,
}

impl api_reads::Endpoint {
    /// Creates only native configuration, without process or filesystem work.
    pub(crate) fn local_shell(&self, shell_path: Option<String>) -> Result<Endpoint, String> {
        self.0.request(|id, reply| {
            Message::BuiltinOperation(Operation::LocalShell(id, shell_path, reply))
        })
    }
}
impl Endpoint {
    /// Starts original local work on the existing pump with independent admission.
    pub(crate) async fn execute(&self, execution: Execution) -> Result<Option<f64>, String> {
        self.0
            .request_async(|id, reply, admission| {
                Message::BuiltinOperation(Operation::LocalExec(id, execution, reply, admission))
            })
            .await
    }
}
impl Registry {
    /// Retains the existing native backend using the application's binary directory.
    pub(super) fn local_shell(
        &self,
        api: u64,
        shell_path: Option<String>,
    ) -> Result<Endpoint, String> {
        let services = self.tool_construction(api)?;
        let operations =
            maestro_extensions::local::create_local_bash_operations(services.bin_dir, shell_path);
        self.insert(Capability::LocalBash(operations)).map(Endpoint)
    }
}
impl State {
    /// Admits raw execution without retaining a borrowed native output sink in the registry.
    pub(super) fn local_exec(
        &self,
        id: u64,
        execution: Execution,
        reply: tokio::sync::oneshot::Sender<Result<Option<f64>, String>>,
        admission: Admission,
    ) {
        let capability = self.registry.capability(id);
        let selected = (|| {
            let Some(Capability::LocalBash(operations)) = capability.as_deref() else {
                return Err("extension local shell resource ended".to_owned());
            };
            let signal = execution
                .signal
                .as_ref()
                .map(|signal| self.registry.cancellation(signal))
                .transpose()?;
            let events = self.registry.events(&execution.api)?;
            Ok((operations.clone(), signal, events))
        })();
        match selected {
            Ok((operations, signal, events)) => {
                let registry = self.registry.clone();
                self.admit(
                    Box::pin(async move {
                        let result = execution.run(operations, signal, (registry, events)).await;
                        let _ = reply.send(result);
                    }),
                    admission,
                );
            }
            Err(error) => {
                let _ = reply.send(Err(error));
            }
        }
    }
}

impl Execution {
    /// Borrows the native sink only while the original backend drains and reaps its child.
    async fn run(
        self,
        operations: Rc<dyn BashOperations<Local>>,
        signal: Option<Cancellation>,
        (registry, events): (Registry, EventBus<Local>),
    ) -> Result<Option<f64>, String> {
        let request = self.request;
        let env = request.env.map(|env| {
            env.into_iter()
                .map(|(key, value)| (key.into(), value.map(Into::into)))
                .collect()
        });
        let mut on_data = |bytes: &[u8]| report_output(&registry, &events, &self.output, bytes);
        operations
            .exec(
                &request.command,
                &request.cwd,
                BashExecOptions {
                    on_data: &mut on_data,
                    signal: signal.as_ref(),
                    timeout: request.timeout,
                    env: env.as_ref(),
                },
            )
            .await
            .map(|result| result.exit_code)
            .map_err(|error| error.to_string())
    }
}

/// Reports a raw observer rejection through the existing event bus and keeps native settlement.
fn report_output(registry: &Registry, events: &EventBus<Local>, output: &Handle, bytes: &[u8]) {
    let result = serde_json::to_string(bytes)
        .map_err(|error| error.to_string())
        .and_then(|data| {
            crate::compact_calls::notify(output, registry, events, SynchronousKind::Listener, data)
        });
    if let Err(error) = result {
        events.report_error("bash-output", &std::io::Error::other(error));
    }
}
