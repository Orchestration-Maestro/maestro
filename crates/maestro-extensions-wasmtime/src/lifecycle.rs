//! One per-extension Store owns entry, cached faults and quiescent destruction.
use crate::{
    api_reads::Endpoint,
    guest_calls::{Call, Callbacks},
    guest_exports::Exports,
};
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, WeakUnboundedSender};
use wasmtime::{
    Store,
    component::{Linker, ResourceTable},
};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};

mod completion;
mod endpoint;
pub(crate) use endpoint::{Endpoint as StoreEndpoint, GuestResource};
mod invocation;
mod operands;
pub(crate) mod replies;

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
    fn callbacks(&mut self) -> &mut Callbacks {
        &mut self.callbacks
    }
}

/// Installs each canonical import once against its existing domain owner.
pub(crate) fn link(linker: &mut Linker<State>) -> wasmtime::Result<()> {
    wasmtime_wasi::p2::add_to_linker_async(&mut *linker)?;
    let mut host = linker.instance("maestro:extension/host@0.1.0")?;
    crate::api_imports::add_to_linker(&mut host)?;
    crate::callback_imports::add_to_linker(&mut host)?;
    crate::context_imports::add_to_linker(&mut host)?;
    crate::ui_imports::add_to_linker(&mut host)?;
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
    crate::registration_imports::add_to_linker(&mut host)
}

/// Drives admitted exports; only this owner may leave concurrency to destroy resources.
pub(crate) async fn drive(
    store: &mut Store<State>,
    exports: Arc<Exports>,
    incoming: &mut UnboundedReceiver<Call>,
    calls: WeakUnboundedSender<Call>,
) -> wasmtime::Result<()> {
    let mut activity = Activity::default();
    loop {
        let ended = store
            .run_concurrent(async |accessor| {
                turn(accessor, &exports, incoming, &calls, &mut activity).await
            })
            .await??;
        for resource in activity.releases.drain(..) {
            resource.resource_drop_async(&mut *store).await?;
        }
        if ended {
            return Ok(());
        }
    }
}

/// Quiescence bookkeeping and the single guest-resource destruction queue.
#[derive(Default)]
struct Activity {
    /// Includes nested calls until their completion-before-reply notification.
    active: usize,
    /// Only the owning Store may destroy these resources, outside concurrency.
    releases: Vec<wasmtime::component::ResourceAny>,
}
/// Runs one concurrent turn, ending only on unload or healthy quiescence.
async fn turn(
    accessor: &wasmtime::component::Accessor<State>,
    exports: &Arc<Exports>,
    incoming: &mut UnboundedReceiver<Call>,
    calls: &WeakUnboundedSender<Call>,
    activity: &mut Activity,
) -> wasmtime::Result<bool> {
    while let Some(call) = incoming.recv().await {
        match call {
            Call::Finished => activity.active -= 1,
            Call::ReleaseResource(resource) => activity.releases.push(resource),
            call => {
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
                    async move |accessor: &wasmtime::component::Accessor<State>| {
                        invocation.run(accessor).await
                    },
                )?;
            }
        }
        if activity.active == 0 && !activity.releases.is_empty() {
            return Ok(false);
        }
    }
    Ok(true)
}

/// After a fault retain registrations, but never enter guest code or guest destructors again.
pub(crate) async fn faulted(incoming: &mut UnboundedReceiver<Call>) {
    while incoming.recv().await.is_some() {}
}
