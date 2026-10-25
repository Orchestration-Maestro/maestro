//! A native operation in progress hands its nested same-instance calls to its own guest.
//!
//! The Store thread cannot enter an instance that is already waiting inside a synchronous
//! import, so a call raised from that import's native code travels back through the import as
//! a typed step and runs the same guest body the ordinary export runs.
use crate::{
    guest_calls::{
        Call, Callback, SynchronousKind,
        renderers::{ComponentCall, ComponentMethod},
        tools::ToolCall,
    },
    lifecycle::completion::Completion,
    local_resources::effects::{EmitterEndpoint, ThemeEndpoint},
};
use std::sync::mpsc::{Receiver, Sender};
use wasmtime::{
    StoreContextMut,
    component::{ComponentType, Lift, LinkerInstance, Lower, Resource, ResourceAny, ResourceTable},
};

/// A nested same-instance call; its strong ownership travels in its completion.
pub(crate) type Nested = (Call<Callback, ResourceAny>, Completion);

/// Store-side ticket for one native operation in progress.
pub(crate) struct Route {
    /// Nested calls of the operation, closed when the operation returns.
    nested: Receiver<Nested>,
    /// The operation's own result, sent before its nested calls close.
    result: Receiver<Result<(), String>>,
    /// Outstanding nested call, settled exactly once by the guest.
    pending: Option<Nested>,
}

impl Route {
    /// Opens the nested-call channel beside the operation's own reply.
    pub(crate) fn new(result: Receiver<Result<(), String>>) -> (Sender<Nested>, Self) {
        let (send, nested) = std::sync::mpsc::channel();
        let route = Self {
            nested,
            result,
            pending: None,
        };
        (send, route)
    }

    /// Returns a catchable failure discovered before native admission.
    pub(crate) fn failed(error: String) -> Self {
        let (send, result) = std::sync::mpsc::channel();
        let (_, route) = Self::new(result);
        let _ = send.send(Err(error));
        route
    }

    /// Waits for the next nested call, or for the operation's own result once it returned.
    pub(crate) fn next(&mut self) -> Result<Nested, Result<(), String>> {
        self.nested.recv().map_err(|_| {
            self.result
                .recv()
                .unwrap_or_else(|_| Err("extension resource reply ended".to_owned()))
        })
    }
}

#[cfg(test)]
impl Route {
    /// Waits for an operation that makes no nested call into this instance.
    pub(crate) fn result(mut self) -> Result<(), String> {
        match self.next() {
            Ok(_) => Err("unexpected nested call".to_owned()),
            Err(result) => result,
        }
    }
}

/// Owned operands of one nested renderer call.
#[derive(ComponentType, Lower)]
#[component(record)]
struct LocalRenderer {
    /// Authored identity in the shared callback inventory.
    target: u32,
    /// Complete custom-message document.
    message: String,
    /// Actual caller expansion choice.
    expanded: bool,
    /// Transferred theme capability.
    theme: Resource<ThemeEndpoint>,
    /// Independent native invocation frame.
    emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested component render.
#[derive(ComponentType, Lower)]
#[component(record)]
struct LocalRender {
    /// Authored identity of the retained component.
    target: u32,
    /// Actual caller viewport width.
    width: u32,
    /// Independent native invocation frame.
    emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested input or invalidation.
#[derive(ComponentType, Lower)]
#[component(record)]
struct LocalUnit {
    /// Authored identity of the retained component.
    target: u32,
    /// Literal input, or absence for invalidation.
    data: Option<String>,
    /// Independent native invocation frame.
    emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested key-release query.
#[derive(ComponentType, Lower)]
#[component(record)]
struct LocalKey {
    /// Authored identity of the retained component.
    target: u32,
    /// Independent native invocation frame.
    emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested argument preparation.
#[derive(ComponentType, Lower)]
#[component(record)]
struct LocalPrepare {
    /// Authored identity of the tool preparation callback.
    target: u32,
    /// Complete argument document.
    args: String,
    /// Independent native invocation frame.
    emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested synchronous callback prefix.
#[derive(ComponentType, Lower)]
#[component(record)]
struct LocalSynchronous {
    /// Authored identity of the callback.
    target: u32,
    /// Authored callback export.
    kind: SynchronousKind,
    /// Exact data document.
    data: String,
    /// Independent native invocation frame.
    emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested schema method on the still-borrowed native operand.
#[derive(ComponentType, Lower)]
#[component(record)]
struct LocalSchema {
    /// Authored identity of the retained schema node.
    target: u32,
    /// Exact author method requested by the checker.
    operation: crate::schema_wire::Operation,
    /// Invocation-local borrowed operand capability.
    node: Resource<crate::schema_reads::Endpoint>,
    /// Independent native invocation frame.
    emitter: Resource<EmitterEndpoint>,
}

/// The nested same-instance target and its owned operands.
#[derive(ComponentType, Lower)]
#[component(variant)]
enum LocalCall {
    /// A message renderer.
    #[component(name = "renderer")]
    Renderer(LocalRenderer),
    /// A retained component render.
    #[component(name = "render")]
    Render(LocalRender),
    /// A retained component input or invalidation.
    #[component(name = "unit")]
    Unit(LocalUnit),
    /// A retained component key-release query.
    #[component(name = "key")]
    Key(LocalKey),
    /// Tool argument preparation.
    #[component(name = "prepare")]
    Prepare(LocalPrepare),
    /// A synchronous callback prefix.
    #[component(name = "synchronous")]
    Synchronous(LocalSynchronous),
    /// A schema method against the original borrowed operand.
    #[component(name = "schema")]
    Schema(LocalSchema),
}

/// The nested call's own result, returned to the native caller that asked for it.
#[derive(ComponentType, Lower, Lift)]
#[component(variant)]
enum LocalResult {
    /// Optional retained component identity, or the authored error.
    #[component(name = "renderer")]
    Renderer(Result<Option<Resource<Callback>>, String>),
    /// Exact rendering lines or the authored error.
    #[component(name = "render")]
    Render(Result<Vec<String>, String>),
    /// Unit result or the authored error.
    #[component(name = "unit")]
    Unit(Result<(), String>),
    /// Key-release preference or the authored error.
    #[component(name = "key")]
    Key(Result<bool, String>),
    /// Prepared argument document or the authored error.
    #[component(name = "prepared")]
    Prepared(Result<String, String>),
    /// Unit result of a synchronous prefix, or the authored error.
    #[component(name = "synchronous")]
    Synchronous(Result<(), String>),
    /// The schema result with every authored diagnostic field.
    #[component(name = "schema")]
    Schema(Result<String, crate::schema_wire::Error>),
}

/// One step of a native operation, as the guest receives it.
#[derive(ComponentType, Lower)]
#[component(variant)]
enum RouteStep {
    /// A nested same-instance call to run now.
    #[component(name = "invoke")]
    Invoke(LocalCall),
    /// The operation's own final result.
    #[component(name = "done")]
    Done(Result<(), String>),
}

/// Installs the progress ticket on the original host interface.
pub(crate) fn link<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::context_imports::add_resource::<T, Route>(host, "import-route")?;
    host.func_wrap(
        "[method]import-route.next",
        |mut store: StoreContextMut<'_, T>, (resource,): (Resource<Route>,)| {
            let next = store.data_mut().ctx().table.get_mut(&resource)?.next();
            let step = match next {
                Ok(nested) => {
                    let table = store.data_mut().ctx().table;
                    let local = local_call(table, &nested.0)?;
                    table.get_mut(&resource)?.pending = Some(nested);
                    RouteStep::Invoke(local)
                }
                Err(result) => RouteStep::Done(result),
            };
            Ok((step,))
        },
    )?;
    host.func_wrap(
        "[method]import-route.resume",
        |mut store: StoreContextMut<'_, T>, (resource, value): (Resource<Route>, LocalResult)| {
            resume(&mut store, &resource, value)
        },
    )?;
    Ok(())
}

/// Pushes a native operation's ticket for the guest to drive.
pub(crate) fn push<T: wasmtime_wasi::WasiView>(
    store: &mut T,
    route: Route,
) -> wasmtime::Result<Resource<Route>> {
    Ok(store.ctx().table.push(route)?)
}

/// Settles exactly the native call this route is waiting for.
fn resume<T: crate::callback_imports::CallbackView>(
    store: &mut StoreContextMut<'_, T>,
    resource: &Resource<Route>,
    value: LocalResult,
) -> wasmtime::Result<()> {
    let pending = store
        .data_mut()
        .ctx()
        .table
        .get_mut(resource)?
        .pending
        .take();
    let (call, _completion) =
        pending.ok_or_else(|| wasmtime::format_err!("extension route has no invocation"))?;
    match (call, value) {
        (Call::Renderer(call), LocalResult::Renderer(value)) => {
            let value = match value {
                Ok(component) => Ok(component
                    .map(|component| {
                        crate::callback_imports::transferred(store.data_mut(), component)
                    })
                    .transpose()?),
                Err(error) => Err(error),
            };
            let _ = call.reply.send(value);
        }
        (Call::Component(ComponentCall::Render(_, _, _, reply)), LocalResult::Render(value)) => {
            let _ = reply.send(value);
        }
        (Call::Component(ComponentCall::Unit(_, _, _, reply)), LocalResult::Unit(value)) => {
            let _ = reply.send(value);
        }
        (Call::Component(ComponentCall::WantsKeyRelease(_, _, reply)), LocalResult::Key(value)) => {
            let _ = reply.send(value);
        }
        (Call::Tool(ToolCall::Prepare { reply, .. }), LocalResult::Prepared(value)) => {
            let _ = reply.send(value);
        }
        (Call::Schema(call), LocalResult::Schema(value)) => {
            let _ = call.reply.send(Ok(value));
        }
        (Call::Synchronous(call), LocalResult::Synchronous(value)) => {
            let _ = call.completion.send(value);
        }
        _ => {
            return Err(wasmtime::format_err!(
                "extension route result kind mismatch"
            ));
        }
    }
    Ok(())
}

/// Marshals owned operands without entering a new canonical guest activation.
fn local_call(
    table: &mut ResourceTable,
    call: &Call<Callback, ResourceAny>,
) -> wasmtime::Result<LocalCall> {
    Ok(match call {
        Call::Renderer(call) => LocalCall::Renderer(LocalRenderer {
            target: call.handler.id,
            message: call.message.clone(),
            expanded: call.expanded,
            theme: table.push(call.theme.clone())?,
            emitter: table.push(call.emitter.clone())?,
        }),
        Call::Component(ComponentCall::Render(owner, width, emitter, _)) => {
            LocalCall::Render(LocalRender {
                target: owner.id,
                width: *width,
                emitter: table.push(emitter.clone())?,
            })
        }
        Call::Component(ComponentCall::Unit(owner, method, emitter, _)) => {
            LocalCall::Unit(LocalUnit {
                target: owner.id,
                data: match method {
                    ComponentMethod::Input(data) => Some(data.clone()),
                    ComponentMethod::Invalidate => None,
                },
                emitter: table.push(emitter.clone())?,
            })
        }
        Call::Component(ComponentCall::WantsKeyRelease(owner, emitter, _)) => {
            LocalCall::Key(LocalKey {
                target: owner.id,
                emitter: table.push(emitter.clone())?,
            })
        }
        Call::Tool(ToolCall::Prepare {
            handler,
            args,
            emitter,
            ..
        }) => LocalCall::Prepare(LocalPrepare {
            target: handler.id,
            args: args.clone(),
            emitter: table.push(emitter.clone())?,
        }),
        Call::Schema(call) => LocalCall::Schema(LocalSchema {
            target: call.handler.id,
            operation: call.operation.clone(),
            node: table.push(call.node.clone())?,
            emitter: table.push(call.emitter.clone())?,
        }),
        Call::Synchronous(call) => LocalCall::Synchronous(LocalSynchronous {
            target: call.handler.id,
            kind: call.kind,
            data: call.data.clone(),
            emitter: table.push(call.emitter.clone())?,
        }),
        _ => {
            return Err(wasmtime::format_err!(
                "extension nested target is not synchronous"
            ));
        }
    })
}
