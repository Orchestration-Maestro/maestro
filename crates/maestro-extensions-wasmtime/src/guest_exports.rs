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
mod completions;
mod credentials;
pub(crate) mod custom;
mod editors;
pub(crate) mod events;
mod factories;
mod file_operations;
mod model_hooks;
mod oauth;
mod paints;
mod providers;
mod widget_calls;
pub(crate) use factories::{rendered_owners, retained};
pub(crate) mod registrations;
mod renderers;
pub(crate) mod schema;
mod streams;
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
    /// Actual OAuth interaction callbacks.
    oauth: oauth::Exports,
    /// Actual credential backend and fallback callbacks.
    credentials: credentials::Exports,
    /// Original authored factory entry point.
    start: TypedFunc<(), (Result<(), String>,)>,
    /// The original retained pub.
    pub(crate) schema: schema::Exports,
    /// Awaited hooks on the native invocation.
    model_hooks: model_hooks::Exports,
    /// Registered setup and original returned-stream operations.
    providers: providers::Exports,
    /// Actual synchronous authored-stream callbacks.
    streams: streams::Exports,
    /// Actual renderer and retained component methods.
    renderers: renderers::Exports,
    /// Actual returned shell operation method.
    bash: bash::Exports,
    /// Actual reverse read operation export.
    read: file_operations::Export<
        crate::guest_calls::read::Operation,
        crate::guest_calls::read::Output,
    >,
    /// Actual reverse write effect export.
    write: file_operations::Export<crate::guest_calls::write::Operation, ()>,
    /// Original reverse edit effects and their diagnostic fields.
    edit: file_operations::Export<
        crate::guest_calls::edit::Operation,
        crate::guest_calls::read::Output,
    >,
    /// Original content search observations and failures.
    grep: file_operations::Export<
        crate::guest_calls::grep::Operation,
        crate::guest_calls::grep::Output,
    >,
    /// Original directory observations and structured failures.
    ls: file_operations::Export<crate::guest_calls::ls::Operation, crate::guest_calls::ls::Output>,
    /// Original glob observations and structured failures.
    find: file_operations::Export<
        crate::guest_calls::find::Operation,
        crate::guest_calls::find::Output,
    >,
    /// Actual concrete event export.
    events: events::Exports,
    /// Actual preparation and execution exports.
    tools: tools::Exports,
    /// Actual terminal input export.
    terminal: terminal::Exports,
    /// Actual component factory export.
    factories: factories::Exports,
    /// Actual custom prefix and owned continuation exports.
    custom: custom::Exports,
    /// Authored styling and highlighting exports.
    paints: paints::Exports,
    /// Authored widget callback export.
    widgets: widget_calls::Exports,
    /// Editor factory and authored editor exports.
    editors: editors::Exports,
    /// Completion provider and provider factory exports.
    completions: completions::Exports,
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
    /// Native file notification callback.
    watch: Abort,
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
        let widgets = widget_calls::Exports::new(store, instance, &guest)?;
        let editors = editors::Exports::new(store, instance, &guest)?;
        let completions = completions::Exports::new(store, instance, &guest)?;
        let bash = bash::Exports::new(store, instance, &guest)?;
        let events = events::Exports::new(store, instance, &guest)?;
        let renderers = renderers::Exports::new(store, instance, &guest)?;
        Ok(Self {
            oauth: oauth::Exports::new(store, instance, &guest)?,
            credentials: credentials::Exports::new(store, instance, &guest)?,
            renderers,
            streams: streams::Exports::new(store, instance, &guest)?,
            model_hooks: model_hooks::Exports::new(store, instance, &guest)?,
            read: typed(store, instance, &guest, "invoke-read")?,
            write: typed(store, instance, &guest, "invoke-write")?,
            edit: typed(store, instance, &guest, "invoke-edit")?,
            ls: typed(store, instance, &guest, "invoke-ls")?,
            find: typed(store, instance, &guest, "invoke-find")?,
            grep: typed(store, instance, &guest, "invoke-grep")?,
            providers: providers::Exports::new(store, instance, &guest)?,
            schema: schema::Exports::new(store, instance, &guest)?,
            start: typed(store, instance, &guest, "start")?,
            events,
            bash,
            tools,
            terminal,
            factories,
            custom: custom::Exports::new(store, instance, &guest)?,
            paints,
            widgets,
            editors,
            completions,
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
            watch: typed(store, instance, &guest, "invoke-watch")?,
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
            call @ (Call::Read(_)
            | Call::Write(_)
            | Call::Edit(_)
            | Call::Find(_)
            | Call::Ls(_)
            | Call::Grep(_)
            | Call::Bash(_)) => {
                self.io(accessor, call).await?;
            }
            call @ (Call::Stream(_)
            | Call::ModelHook(_)
            | Call::Schema(_)
            | Call::OAuth(_)
            | Call::Provider(_)
            | Call::Assistant(_)) => {
                self.invoke_value_callback(accessor, call).await?;
            }
            Call::Credential(call) => self.credentials.invoke(accessor, call).await?,
            Call::Tool(call) => self.tools.invoke(accessor, call).await?,
            Call::Registered(call) => self.registered.invoke(accessor, call).await?,
            call @ (Call::TerminalInput(_)
            | Call::Factory(_)
            | Call::Custom(_)
            | Call::ToolRender(_)
            | Call::Paint(_)
            | Call::Widget(_)
            | Call::EditorFactory(_)
            | Call::Editor(_)
            | Call::AutocompleteFactory(_)
            | Call::Autocomplete(_)) => {
                self.ui(accessor, call).await?;
            }
            Call::Synchronous(call) => self.prefix(accessor, call).await?,
            Call::Tail(handler, reply) => self.tail(accessor, &handler, reply).await?,

            call @ (Call::Setup(..) | Call::WithSession(..)) => {
                self.session(accessor, call).await?;
            }
            Call::Release(release) => self.release(accessor, release).await?,
        }
        Ok(())
    }
    /// Enters one session setup callback with its retained session capability.
    async fn session<T: crate::callback_imports::CallbackView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: Call<Callback>,
    ) -> wasmtime::Result<()> {
        match call {
            Call::Setup(handler, writer, reply) => {
                let result = session(accessor, self.setup, handler, writer).await?;
                let _ = reply.send(result);
            }
            Call::WithSession(handler, context, reply) => {
                let result = session(accessor, self.with_session, handler, context).await?;
                let _ = reply.send(result);
            }
            _ => return Err(wasmtime::format_err!("expected a session callback")),
        }
        Ok(())
    }
    /// Invoke a value callback through its original typed export.
    async fn invoke_value_callback<T: crate::callback_imports::CallbackView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: Call<Callback>,
    ) -> wasmtime::Result<()> {
        match call {
            Call::Provider(call) => self.providers.invoke(accessor, call).await,
            Call::Assistant(call) => self.providers.assistant(accessor, call).await,
            Call::Stream(call) => self.streams.invoke(accessor, call).await,
            Call::ModelHook(call) => self.model_hooks.invoke(accessor, call).await,
            Call::Schema(call) => self.schema.invoke(accessor, call).await,
            Call::OAuth(call) => self.oauth.invoke(accessor, call).await,
            _ => Err(wasmtime::format_err!("extension call cannot run nested")),
        }
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
            Call::Custom(call) => self.custom.invoke(accessor, call).await,
            Call::ToolRender(call) => self.factories.render(accessor, call).await,
            Call::Paint(call) => self.paints.invoke(accessor, call).await,
            Call::Widget(call) => self.widgets.invoke(accessor, call).await,
            Call::EditorFactory(call) => self.editors.factory(accessor, call).await,
            Call::Editor(call) => self.editors.request(accessor, call).await,
            Call::AutocompleteFactory(call) => self.completions.factory(accessor, call).await,
            Call::Autocomplete(call) => self.completions.request(accessor, call).await,
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
        let callback = accessor.with(|mut access| {
            access.get().ctx().table.push(Callback {
                id: handler.id,
                schema: None,
            })
        })?;
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
            let handler = accessor
                .with(|mut access| access.get().ctx().table.push(Callback { id, schema: None }))?;
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
                    schema: None,
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
            SynchronousKind::Abort
            | SynchronousKind::Dispose
            | SynchronousKind::BranchChange
            | SynchronousKind::Watch => {
                let export = match call.kind {
                    SynchronousKind::Abort => self.abort,
                    SynchronousKind::Watch => self.watch,
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
        Ok((
            table.push(Callback {
                id: handler.id,
                schema: None,
            })?,
            table.push(value)?,
        ))
    })
}

impl Exports {
    /// Enters original content and path observations through their typed exports.
    async fn search<T: crate::callback_imports::CallbackView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: Call<Callback>,
    ) -> wasmtime::Result<()> {
        match call {
            Call::Grep(call) => {
                file_operations::invoke(
                    &self.grep,
                    accessor,
                    call.handler,
                    (call.operation, call.path),
                    call.reply,
                )
                .await
            }
            Call::Ls(call) => {
                file_operations::invoke(
                    &self.ls,
                    accessor,
                    call.handler,
                    (call.operation, call.path),
                    call.reply,
                )
                .await
            }
            Call::Find(call) => {
                file_operations::invoke(
                    &self.find,
                    accessor,
                    call.handler,
                    (call.operation, call.path),
                    call.reply,
                )
                .await
            }
            _ => wasmtime::bail!("extension search call has no export"),
        }
    }
}
