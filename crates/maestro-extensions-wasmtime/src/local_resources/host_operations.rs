//! Factory-time operations use the same admitted local pump as session calls.
use super::{Admission, Capability, Message, pump::State};
use maestro_extensions::host_operations::{HostOperation, OperationScope};
use wasmtime::component::{ComponentType, Lift, LinkerInstance};

/// Canonical installed scope, with no temporary or caller-selected target.
#[derive(ComponentType, Lift)]
#[component(enum)]
#[repr(u8)]
#[expect(dead_code, reason = "constructed by Wasmtime Lift from wire values")]
enum Scope {
    /// User configuration.
    #[component(name = "user")]
    User,
    /// Project configuration.
    #[component(name = "project")]
    Project,
}
impl From<Scope> for OperationScope {
    fn from(scope: Scope) -> Self {
        match scope {
            Scope::User => Self::User,
            Scope::Project => Self::Project,
        }
    }
}
/// Exact source spelling and its independent installed scope.
#[derive(ComponentType, Lift)]
#[component(record)]
struct Package {
    /// Authored source including its pin.
    source: String,
    /// Installed location policy.
    scope: Scope,
}
/// Sources the factory selected itself and their one installed scope.
#[derive(ComponentType, Lift)]
#[component(record)]
struct Sources {
    /// Authored sources including their pins.
    sources: Vec<String>,
    /// Installed location policy shared by every source.
    scope: Scope,
}
/// Canonical domain requests, not an untyped operation-name dispatcher.
#[derive(ComponentType, Lift)]
#[component(variant)]
enum Operation {
    /// Raw document read.
    #[component(name = "read-settings")]
    ReadSettings(Scope),
    /// Configured, or explicitly supplied, package resolution.
    #[component(name = "resolve-packages")]
    ResolvePackages(Option<Sources>),
    /// Pinned acquisition.
    #[component(name = "acquire")]
    Acquire(Package),
    /// Nonmutating integrity observation.
    #[component(name = "verify-pin")]
    VerifyPin(Package),
    /// Native clean-pin restoration.
    #[component(name = "restore-pin")]
    RestorePin(Package),
}
impl From<Operation> for HostOperation {
    fn from(operation: Operation) -> Self {
        match operation {
            Operation::ReadSettings(scope) => Self::ReadSettings(scope.into()),
            Operation::ResolvePackages(explicit) => Self::ResolvePackages(
                explicit.map(|sources| (sources.sources, sources.scope.into())),
            ),
            Operation::Acquire(package) => Self::Acquire(package.source, package.scope.into()),
            Operation::VerifyPin(package) => Self::VerifyPin(package.source, package.scope.into()),
            Operation::RestorePin(package) => {
                Self::RestorePin(package.source, package.scope.into())
            }
        }
    }
}
/// One admitted request with its API identity and reply.
pub(crate) struct Call {
    /// The API resource whose original facade answers.
    id: u64,
    /// The selected domain request.
    operation: HostOperation,
    /// Serialized native result or native failure text.
    reply: tokio::sync::oneshot::Sender<Result<String, String>>,
    /// Keeps the pump alive until the operation settles.
    admission: Admission,
}
/// Installs the asynchronous domain call without another request receiver.
pub(crate) fn add_to_linker<T: crate::api_imports::ApiView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap_concurrent(
        "call-host-operation",
        |accessor, (operation,): (Operation,)| {
            Box::pin(async move {
                let api = accessor.with(|mut access| access.get().api().clone());
                let result = api
                    .0
                    .request_async(|id, reply, admission| {
                        Message::HostOperation(Call {
                            id,
                            operation: operation.into(),
                            reply,
                            admission,
                        })
                    })
                    .await;
                Ok((result,))
            })
        },
    )
}
impl State {
    /// Selects the original operation before releasing the API capability.
    pub(super) fn host_operation(&self, call: Call) {
        let Call {
            id,
            operation,
            reply,
            admission,
        } = call;
        let capability = self.registry.capability(id);
        let Some(Capability::Api(api)) = capability.as_deref() else {
            let _ = reply.send(Err("extension API resource ended".into()));
            return;
        };
        let work = api.host_operation(operation);
        self.admit(
            Box::pin(async move {
                let result = work.await.and_then(|value| {
                    serde_json::to_string(&value).map_err(|error| error.to_string())
                });
                let _ = reply.send(result);
            }),
            admission,
        );
    }
}
