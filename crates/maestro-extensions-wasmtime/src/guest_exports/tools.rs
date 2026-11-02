//! Canonical tool callbacks transfer their original native invocation capabilities.
use super::arguments;
use crate::{
    context_imports::Context,
    guest_calls::{Callback, ToolCall},
    local_resources::{SignalEndpoint, effects::ToolUpdate},
};
use wasmtime::{
    Store,
    component::{
        Accessor, ComponentExportIndex, ComponentType, Instance, Lift, Lower, Resource, TypedFunc,
    },
};
use wasmtime_wasi::WasiView;

/// Exact owned canonical tool invocation record.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(crate) struct Invocation {
    /// Literal invocation identity.
    #[component(name = "tool-call-id")]
    pub(crate) tool_call_id: String,
    /// Serialized native arguments.
    pub(crate) params: String,
}

/// Independently retained original execution resources.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(crate) struct Capabilities {
    /// Original wrapper-created context.
    pub(crate) ctx: Resource<Context>,
    /// Independently optional native cancellation.
    pub(crate) signal: Option<Resource<SignalEndpoint>>,
    /// Independently optional progress observer.
    pub(crate) update: Option<Resource<ToolUpdate>>,
}

/// Retained mutations remain independent of the prepared value or callback failure.
#[derive(ComponentType, Lift, Lower, Debug)]
#[component(record)]
pub(crate) struct PreparedArguments {
    /// Original arguments after the callback's in-place changes.
    pub(crate) retained: String,
    /// Returned preparation or the callback's original failure.
    pub(crate) prepared: Result<String, String>,
}

/// Exact canonical preparation signature, returning both argument observations.
type Prepare = TypedFunc<
    (
        Resource<Callback>,
        String,
        Resource<crate::local_resources::effects::EmitterEndpoint>,
    ),
    (Result<PreparedArguments, String>,),
>;
/// Canonical asynchronous tool execution signature.
type Execute = TypedFunc<(Resource<Callback>, Invocation, Capabilities), (Result<String, String>,)>;

/// Required tool exports from the same canonical guest instance.
pub(super) struct Exports {
    /// Synchronous preparation callback.
    prepare: Prepare,
    /// Asynchronous execution callback.
    execute: Execute,
}

impl Exports {
    /// Resolves real guest exports, without an alternate implementation.
    pub(super) fn new<T: 'static>(
        store: &mut Store<T>,
        instance: &Instance,
        guest: &ComponentExportIndex,
    ) -> wasmtime::Result<Self> {
        Ok(Self {
            prepare: super::typed(store, instance, guest, "invoke-prepare")?,
            execute: super::typed(store, instance, guest, "invoke-tool")?,
        })
    }

    /// Enters the actual authored callback through the active Accessor.
    pub(super) async fn invoke<T: WasiView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: ToolCall<Callback>,
    ) -> wasmtime::Result<()> {
        match call {
            ToolCall::Prepare {
                handler,
                args,
                emitter,
                reply,
            } => {
                let result = self.prepared(accessor, &handler, args, emitter).await?;
                let _ = reply.send(result);
            }
            ToolCall::Execute {
                handler,
                id,
                params,
                context,
                signal,
                update,
                reply,
            } => {
                let (callback, ctx) = arguments(accessor, &handler, Context(context))?;
                let resources = capabilities(accessor, ctx, signal, update)?;
                let result = self
                    .execute
                    .call_concurrent(
                        accessor,
                        (
                            Resource::new_borrow(callback.rep()),
                            Invocation {
                                tool_call_id: id,
                                params,
                            },
                            resources,
                        ),
                    )
                    .await;
                accessor.with(|mut access| access.get().ctx().table.delete(callback))?;
                let _ = reply.send(result?.0);
            }
        }
        Ok(())
    }
    /// Runs one preparation under the original invocation frame, deleting temporary borrows.
    async fn prepared<T: WasiView + 'static>(
        &self,
        accessor: &Accessor<T>,
        handler: &Callback,
        args: String,
        emitter: crate::local_resources::effects::EmitterEndpoint,
    ) -> wasmtime::Result<Result<PreparedArguments, String>> {
        let (callback, emitter) = arguments(accessor, handler, emitter)?;
        let result = self
            .prepare
            .call_concurrent(
                accessor,
                (
                    Resource::new_borrow(callback.rep()),
                    args,
                    Resource::new_borrow(emitter.rep()),
                ),
            )
            .await;
        accessor.with(|mut access| {
            let table = access.get().ctx().table;
            table.delete(callback)?;
            table.delete(emitter)?;
            Ok::<_, wasmtime::Error>(())
        })?;
        Ok(result?.0)
    }
}

/// Pushes independent execution operands into the same canonical resource table.
fn capabilities<T: WasiView + 'static>(
    accessor: &Accessor<T>,
    ctx: Resource<Context>,
    signal: Option<SignalEndpoint>,
    update: Option<ToolUpdate>,
) -> wasmtime::Result<Capabilities> {
    accessor.with(|mut access| {
        let table = access.get().ctx().table;
        Ok(Capabilities {
            ctx,
            signal: signal.map(|signal| table.push(signal)).transpose()?,
            update: update.map(|update| table.push(update)).transpose()?,
        })
    })
}
