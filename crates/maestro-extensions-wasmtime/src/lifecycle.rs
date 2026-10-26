//! One per-extension Store owns entry, cached faults and quiescent destruction.
use crate::{
    api_reads::Endpoint,
    guest_calls::{Call, Callbacks},
    guest_exports::Exports,
};
use futures_util::future::{Either, select};
use std::sync::Arc;
use tokio::sync::{
    Notify,
    mpsc::{UnboundedReceiver, WeakUnboundedSender},
};
use wasmtime::{
    Store,
    component::{Linker, ResourceTable},
};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};

pub(crate) mod completion;
mod endpoint;
pub(crate) use endpoint::Endpoint as StoreEndpoint;
mod invocation;
mod operands;
pub(crate) mod replies;

/// Store calls share their existing receiver with the native owner's retirement signal.
pub(crate) struct Incoming {
    /// The sole queue for admitted guest calls and final releases.
    calls: UnboundedReceiver<Call>,
    /// The pump holds only a weak notification reference.
    shutdown: Arc<Notify>,
    /// Retirement drains queued calls once active work reaches quiescence.
    ending: bool,
}

impl Incoming {
    /// Keeps retirement available even when instantiation fails before creating a Store.
    pub(crate) fn new(calls: UnboundedReceiver<Call>, shutdown: Arc<Notify>) -> Self {
        Self {
            calls,
            shutdown,
            ending: false,
        }
    }

    /// Receives calls until the native owner retires and admitted work is quiescent.
    async fn recv(&mut self, active: usize) -> Option<Call> {
        if self.ending {
            return self.drain(active).await;
        }
        {
            let next = std::pin::pin!(self.calls.recv());
            let shutdown = std::pin::pin!(self.shutdown.notified());
            if let Either::Left((call, _)) = select(next, shutdown).await {
                return call;
            }
        }
        self.ending = true;
        self.drain(active).await
    }

    /// Drains final calls without waiting once active guest work has ended.
    async fn drain(&mut self, active: usize) -> Option<Call> {
        if active == 0 {
            self.calls.try_recv().ok()
        } else {
            self.calls.recv().await
        }
    }
}

/// Only Store-safe capabilities cross to the dedicated Store thread.
pub(crate) struct State {
    /// WASI configuration belongs to this instance alone.
    wasi: WasiCtx,
    /// Canonical host capabilities retain their original local owners.
    table: ResourceTable,
    /// The original registration facade's endpoint, not a reconstructed API.
    api: Endpoint,
    /// Weak callback registration and the single reply/fault owner.
    callbacks: Callbacks,
    /// The original retained pub.
    pub(crate) exports: Option<Arc<Exports>>,
}
impl State {
    /// Constructs the one instance state before canonical linking.
    ///
    /// The instance reads `path`, the artifact it is loaded from, as `MAESTRO_EXTENSION_PATH`.
    pub(crate) fn new(api: Endpoint, callbacks: Callbacks, path: &str) -> Self {
        Self {
            wasi: wasmtime_wasi::WasiCtxBuilder::new()
                .env("MAESTRO_EXTENSION_PATH", path)
                .build(),
            table: ResourceTable::new(),
            api,
            callbacks,
            exports: None,
        }
    }
}
impl WasiView for State {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}
impl crate::api_imports::ApiView for State {
    fn api(&self) -> &Endpoint {
        &self.api
    }
}
impl crate::callback_imports::CallbackView for State {
    fn exports(&self) -> Result<Arc<Exports>, String> {
        self.exports
            .clone()
            .ok_or_else(|| "extension exports unavailable".into())
    }
    fn callbacks(&mut self) -> &mut Callbacks {
        &mut self.callbacks
    }
}

/// Installs each canonical import once against its existing domain owner.
pub(crate) fn link(linker: &mut Linker<State>) -> wasmtime::Result<()> {
    wasmtime_wasi::p2::add_to_linker_async(&mut *linker)?;
    let mut host = linker.instance("maestro:extension/host@0.1.0")?;
    crate::api_imports::add_to_linker(&mut host)?;
    crate::exec_imports::add_to_linker(&mut host)?;
    crate::callback_imports::add_to_linker(&mut host)?;
    crate::context_imports::add_to_linker(&mut host)?;
    crate::ui_imports::add_to_linker(&mut host)?;
    crate::ui_themes::add_to_linker(&mut host)?;
    crate::footer_data::add_to_linker(&mut host)?;
    crate::theme_imports::add_to_linker(&mut host)?;
    crate::command_imports::add_to_linker(&mut host)?;
    crate::catalog_imports::add_to_linker(&mut host)?;
    crate::history_imports::add_to_linker(&mut host)?;
    crate::tree_imports::add_to_linker(&mut host)?;
    crate::writer_imports::add_to_linker(&mut host)?;
    crate::replaced_imports::add_to_linker(&mut host)?;
    crate::session_imports::add_to_linker(&mut host)?;
    crate::effect_imports::add_to_linker(&mut host)?;
    crate::event_imports::add_to_linker(&mut host)?;
    crate::signal_imports::add_to_linker(&mut host)?;
    crate::schema_imports::add_to_linker(&mut host)?;
    crate::stream_imports::add_to_linker(&mut host)?;
    crate::registration_imports::add_to_linker(&mut host)?;
    crate::import_route::link(&mut host)
}

/// Drives admitted exports until every native owner of this Store has ended.
pub(crate) async fn drive(
    store: &mut Store<State>,
    exports: Arc<Exports>,
    incoming: &mut Incoming,
    calls: WeakUnboundedSender<Call>,
) -> wasmtime::Result<()> {
    let mut activity = Activity::default();
    store
        .run_concurrent(async |accessor| {
            turn(accessor, &exports, incoming, &calls, &mut activity).await
        })
        .await?
}

/// Quiescence bookkeeping and the queue of deferred object destruction.
#[derive(Default)]
struct Activity {
    /// Includes nested calls until their completion-before-reply notification.
    active: usize,
    /// Released authored objects, destroyed once no other call is active.
    objects: Vec<crate::guest_calls::Release>,
}
/// Admits every call in order until the last native owner of this Store ends.
async fn turn(
    accessor: &wasmtime::component::Accessor<State>,
    exports: &Arc<Exports>,
    incoming: &mut Incoming,
    calls: &WeakUnboundedSender<Call>,
    activity: &mut Activity,
) -> wasmtime::Result<()> {
    while let Some(call) = incoming.recv(activity.active).await {
        match call {
            Call::Finished => finished(accessor, exports, calls, activity)?,
            Call::Release(release) if release.object && activity.active > 0 => {
                activity.objects.push(release);
            }
            call => start(accessor, exports, calls, activity, call)?,
        }
    }
    Ok(())
}

/// Counts one completed call and, at quiescence, starts the deferred object releases.
fn finished(
    accessor: &wasmtime::component::Accessor<State>,
    exports: &Arc<Exports>,
    calls: &WeakUnboundedSender<Call>,
    activity: &mut Activity,
) -> wasmtime::Result<()> {
    activity.active -= 1;
    if activity.active > 0 {
        return Ok(());
    }
    for release in std::mem::take(&mut activity.objects) {
        start(accessor, exports, calls, activity, Call::Release(release))?;
    }
    Ok(())
}

/// Admits one call and spawns its guest invocation on the active accessor.
fn start(
    accessor: &wasmtime::component::Accessor<State>,
    exports: &Arc<Exports>,
    calls: &WeakUnboundedSender<Call>,
    activity: &mut Activity,
    call: Call,
) -> wasmtime::Result<()> {
    let sender = match &call {
        Call::Release(release) => release.calls.clone(),
        _ => calls.upgrade(),
    }
    .ok_or_else(|| wasmtime::format_err!("extension Store admission ended"))?;
    let completion = completion::Completion::new(sender);
    call.before(&completion);
    let call = call.admitted(&completion);
    activity.active += 1;
    let invocation = invocation::Invocation {
        exports: exports.clone(),
        call,
        completion,
    };
    accessor.spawn(
        async move |accessor: &wasmtime::component::Accessor<State>| invocation.run(accessor).await,
    )?;
    Ok(())
}

/// After a fault retain registrations, but never enter guest code or guest destructors again.
pub(crate) async fn faulted(incoming: &mut Incoming) {
    while incoming.recv(0).await.is_some() {}
}
