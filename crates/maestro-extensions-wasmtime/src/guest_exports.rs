//! Actual typed guest exports for retained native callbacks.
use crate::guest_calls::{Call, Callback, Synchronous, SynchronousKind};
use wasmtime::{
    Store,
    component::{
        Accessor, ComponentExportIndex, ComponentNamedList, Instance, Lift, Lower, Resource,
        TypedFunc,
    },
};
use wasmtime_wasi::WasiView;

pub(crate) mod bash;
pub(crate) mod events;
mod factories;
mod paints;
pub(crate) use factories::{rendered_owners, retained};
pub(crate) mod registrations;
mod renderers;
pub(crate) mod schema;
mod terminal;
pub(crate) mod tools;

/// Typed session callback with the actual domain capability resource.
type Session<V> = TypedFunc<(Resource<Callback>, Resource<V>), (Result<(), String>,)>;
/// Synchronous closure release export.
type Release = TypedFunc<(Resource<Callback>,), ()>;

/// Actual synchronous prefix with its original borrowed invocation frame.
type Prefix = TypedFunc<
    (
        Resource<Callback>,
        String,
        Resource<crate::local_resources::effects::EmitterEndpoint>,
    ),
    (Result<(), String>,),
>;
/// Independently owned guest listener continuation.
type Tail = TypedFunc<(Resource<Callback>,), (Result<(), String>,)>;
/// Synchronous abort observer with its borrowed invocation frame and no data.
type Abort = TypedFunc<
    (
        Resource<Callback>,
        Resource<crate::local_resources::effects::EmitterEndpoint>,
    ),
    (Result<(), String>,),
>;

/// Exports resolved from the canonical guest interface, never substitute implementations.
pub(crate) struct Exports {
    /// Original authored factory entry point.
    start: TypedFunc<(), (Result<(), String>,)>,
    /// The original retained pub.
    pub(crate) schema: schema::Exports,
    /// Actual renderer and retained component methods.
    renderers: renderers::Exports,
    /// Actual returned shell operation method.
    bash: bash::Exports,
    /// Actual concrete event export.
    events: events::Exports,
    /// Actual preparation and execution exports.
    tools: tools::Exports,
    /// Actual terminal input export.
    terminal: terminal::Exports,
    /// Actual component factory export.
    factories: factories::Exports,
    /// Authored styling and highlighting exports.
    paints: paints::Exports,
    /// Native component disposal.
    dispose: Abort,
    /// Native branch change listener.
    branch_change: Abort,
    /// Actual command, completion and shortcut exports.
    registered: registrations::Exports,
    /// Actual authored setup entry point.
    setup: Session<crate::writer_reads::Endpoint>,
    /// Actual authored replacement continuation.
    with_session: Session<crate::replaced_reads::Endpoint>,
    /// Native event-listener prefix.
    listener: Prefix,
    /// Native compaction success callback.
    compaction_complete: Prefix,
    /// Native compaction failure callback.
    compaction_error: Prefix,
    /// Native abort observer.
    abort: Abort,
    /// Independently owned event continuation.
    tail: Tail,
    /// Actual authored table-release operation.
    release: Release,
}

/// Resolves and types one required export of the canonical guest interface.
fn typed<T: 'static, P, R>(
    store: &mut Store<T>,
    instance: &Instance,
    guest: &ComponentExportIndex,
    name: &str,
) -> wasmtime::Result<TypedFunc<P, R>>
where
    P: ComponentNamedList + Lower,
    R: ComponentNamedList + Lift,
{
    let index = instance
        .get_export_index(&mut *store, Some(guest), name)
        .ok_or_else(|| wasmtime::format_err!("missing extension guest export {name}"))?;
    instance.get_typed_func(store, index)
}

impl Exports {
    /// Resolves required exports from the same instantiated canonical component.
    pub(crate) fn new<T: 'static>(
        store: &mut Store<T>,
        instance: &Instance,
    ) -> wasmtime::Result<Self> {
        let guest = instance
            .get_export_index(&mut *store, None, "maestro:extension/guest@0.1.0")
            .ok_or_else(|| wasmtime::format_err!("missing extension guest interface"))?;
        let registered = registrations::Exports::new(store, instance, &guest)?;
        let tools = tools::Exports::new(store, instance, &guest)?;
        let terminal = terminal::Exports::new(store, instance, &guest)?;
        let factories = factories::Exports::new(store, instance, &guest)?;
        let paints = paints::Exports::new(store, instance, &guest)?;
        let bash = bash::Exports::new(store, instance, &guest)?;
        let events = events::Exports::new(store, instance, &guest)?;
        let renderers = renderers::Exports::new(store, instance, &guest)?;
        Ok(Self {
            renderers,
            schema: schema::Exports::new(store, instance, &guest)?,
            start: typed(store, instance, &guest, "start")?,
            events,
            bash,
            tools,
            terminal,
            factories,
            paints,
            dispose: typed(store, instance, &guest, "invoke-dispose")?,
            branch_change: typed(store, instance, &guest, "invoke-branch-change")?,
            registered,
            setup: typed(store, instance, &guest, "invoke-setup")?,
            with_session: typed(store, instance, &guest, "invoke-with-session")?,
            release: typed(store, instance, &guest, "release")?,
            listener: typed(store, instance, &guest, "invoke-listener")?,
            compaction_complete: typed(store, instance, &guest, "invoke-compaction-complete")?,
            compaction_error: typed(store, instance, &guest, "invoke-compaction-error")?,
            abort: typed(store, instance, &guest, "invoke-abort")?,
            tail: typed(store, instance, &guest, "invoke-tail")?,
        })
    }

    /// Enters the actual authored callback export.
    pub(crate) async fn invoke<T: crate::callback_imports::CallbackView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: Call<Callback>,
    ) -> wasmtime::Result<()> {
        match call {
            Call::Start(reply) => self.start(accessor, reply).await?,
            Call::Finished => {
                return Err(wasmtime::format_err!("completion is not a guest export"));
            }
            Call::Renderer(call) => self.renderers.create(accessor, call).await?,
            Call::Component(call) => self.renderers.component(accessor, call).await?,
            Call::Event(call) => self.events.invoke(accessor, call).await?,
            Call::Bash(call) => self.bash.invoke(accessor, call).await?,
            Call::Schema(call) => self.schema.invoke(accessor, call).await?,
            Call::Tool(call) => self.tools.invoke(accessor, call).await?,
            Call::Registered(call) => self.registered.invoke(accessor, call).await?,
            call @ (Call::TerminalInput(_)
            | Call::Factory(_)
            | Call::ToolRender(_)
            | Call::Paint(_)) => {
                self.ui(accessor, call).await?;
            }
            Call::Synchronous(call) => self.prefix(accessor, call).await?,
            Call::Tail(handler, reply) => self.tail(accessor, &handler, reply).await?,

            Call::Setup(handler, writer, reply) => {
                let result = session(accessor, self.setup, handler, writer).await?;
                let _ = reply.send(result);
            }
            Call::WithSession(handler, context, reply) => {
                let result = session(accessor, self.with_session, handler, context).await?;
                let _ = reply.send(result);
            }
            Call::Release(release) => self.release(accessor, release).await?,
        }
        Ok(())
    }
    /// Enters one terminal input listener or component factory export.
    async fn ui<T: crate::callback_imports::CallbackView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: Call<Callback>,
    ) -> wasmtime::Result<()> {
        match call {
            Call::TerminalInput(call) => self.terminal.invoke(accessor, call).await,
            Call::Factory(call) => self.factories.invoke(accessor, call).await,
            Call::ToolRender(call) => self.factories.render(accessor, call).await,
            Call::Paint(call) => self.paints.invoke(accessor, call).await,
            _ => Err(wasmtime::format_err!("expected a UI callback")),
        }
    }

    /// Enters the authored factory; its failure answers the load before the Store is torn down.
    async fn start<T: WasiView + 'static>(
        &self,
        accessor: &Accessor<T>,
        reply: crate::lifecycle::replies::Reply<()>,
    ) -> wasmtime::Result<()> {
        let result = self.start.call_concurrent(accessor, ()).await?.0;
        if let Err(message) = result {
            let _ = reply.send(Err(message.clone()));
            return Err(wasmtime::Error::msg(message));
        }
        let _ = reply.send(Ok(()));
        Ok(())
    }

    /// Awaits an independently owned listener tail and sends its result.
    async fn tail<T: WasiView + 'static>(
        &self,
        accessor: &Accessor<T>,
        handler: &Callback,
        reply: crate::lifecycle::replies::Reply<()>,
    ) -> wasmtime::Result<()> {
        let callback = accessor
            .with(|mut access| access.get().ctx().table.push(Callback { id: handler.id }))?;
        let result = self
            .tail
            .call_concurrent(accessor, (Resource::new_borrow(callback.rep()),))
            .await;
        accessor.with(|mut access| access.get().ctx().table.delete(callback))?;
        let _ = reply.send(result?.0);
        Ok(())
    }

    /// Retires a callback identity, entering the guest's release export once it is unused.
    async fn release<T: crate::callback_imports::CallbackView + 'static>(
        &self,
        accessor: &Accessor<T>,
        release: crate::guest_calls::Release,
    ) -> wasmtime::Result<()> {
        let retired = accessor.with(|mut access| access.get().callbacks().release(&release));
        if let Some(id) = retired {
            let handler =
                accessor.with(|mut access| access.get().ctx().table.push(Callback { id }))?;
            let result = self
                .release
                .call_concurrent(accessor, (Resource::new_borrow(handler.rep()),))
                .await;
            accessor.with(|mut access| access.get().ctx().table.delete(handler))?;
            result?;
        }
        if let Some(reply) = release.completion {
            let _ = reply.send(Ok(()));
        }
        Ok(())
    }
    /// Keeps the original native frame alive only through its actual guest prefix.
    async fn prefix<T: crate::callback_imports::CallbackView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: Synchronous<Callback>,
    ) -> wasmtime::Result<()> {
        let (callback, emitter) = accessor.with(|mut access| {
            let table = access.get().ctx().table;
            Ok::<_, wasmtime::Error>((
                table.push(Callback {
                    id: call.handler.id,
                })?,
                table.push(call.emitter)?,
            ))
        })?;
        let (handler, frame) = (
            Resource::new_borrow(callback.rep()),
            Resource::new_borrow(emitter.rep()),
        );
        let prefix = match call.kind {
            SynchronousKind::Listener => self.listener,
            SynchronousKind::CompactionComplete => self.compaction_complete,
            SynchronousKind::CompactionError => self.compaction_error,
            SynchronousKind::Abort | SynchronousKind::Dispose | SynchronousKind::BranchChange => {
                let export = match call.kind {
                    SynchronousKind::Abort => self.abort,
                    SynchronousKind::Dispose => self.dispose,
                    _ => self.branch_change,
                };
                let result = export.call_concurrent(accessor, (handler, frame)).await;
                return finish(accessor, (callback, emitter), call.completion, result);
            }
        };
        let result = prefix
            .call_concurrent(accessor, (handler, call.data, frame))
            .await;
        finish(accessor, (callback, emitter), call.completion, result)
    }
}

/// Removes the temporary borrows of one synchronous invocation, then sends its result.
fn finish<T: WasiView + 'static>(
    accessor: &Accessor<T>,
    (callback, emitter): (
        Resource<Callback>,
        Resource<crate::local_resources::effects::EmitterEndpoint>,
    ),
    completion: crate::lifecycle::replies::Reply<()>,
    result: wasmtime::Result<(Result<(), String>,)>,
) -> wasmtime::Result<()> {
    accessor.with(|mut access| {
        let table = access.get().ctx().table;
        table.delete(callback)?;
        table.delete(emitter)?;
        Ok::<_, wasmtime::Error>(())
    })?;
    let _ = completion.send(result?.0);
    Ok(())
}

/// Transfers the supplied native capability and removes only the temporary callback borrow.
async fn session<T: WasiView + 'static, V: Send + 'static>(
    accessor: &Accessor<T>,
    export: Session<V>,
    handler: Callback,
    value: V,
) -> wasmtime::Result<Result<(), String>> {
    let params = arguments(accessor, &handler, value)?;
    let result = export
        .call_concurrent(accessor, (Resource::new_borrow(params.0.rep()), params.1))
        .await;
    accessor.with(|mut access| access.get().ctx().table.delete(params.0))?;
    result.map(|(result,)| result)
}

/// Inserts a temporary callback borrow beside the original transferred native capability.
fn arguments<T: WasiView + 'static, V: Send + 'static>(
    accessor: &Accessor<T>,
    handler: &Callback,
    value: V,
) -> wasmtime::Result<(Resource<Callback>, Resource<V>)> {
    accessor.with(|mut access| {
        let table = access.get().ctx().table;
        Ok((table.push(Callback { id: handler.id })?, table.push(value)?))
    })
}
