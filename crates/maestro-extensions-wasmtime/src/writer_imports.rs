//! Canonical writable setup methods forward to their retained domain session.
use crate::read_imports::ReadEndpoint;
use crate::read_imports::{add_operand, add_read};
use crate::{
    context_imports::add_resource,
    writer_reads::{Endpoint, Request},
};
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};
use wasmtime_wasi::WasiView;

/// Canonical summary operands, including independently optional metadata.
type SummaryArgs = (
    Resource<Endpoint>,
    Option<String>,
    String,
    (Option<String>, Option<bool>),
);

/// Defines all writable operations without a substitute session or publication policy.
pub(crate) fn add_to_linker<T: WasiView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    add_resource::<T, Endpoint>(host, "session-manager")?;
    host.func_wrap(
        "[method]session-manager.reader",
        |mut store: StoreContextMut<'_, T>, (resource,): (Resource<Endpoint>,)| {
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            let reader = endpoint.reader().map_err(wasmtime::Error::msg)?;
            Ok((store.data_mut().ctx().table.push(reader)?,))
        },
    )?;
    add_read(
        host,
        "[method]session-manager.is-persisted",
        Request::Persisted,
        crate::read_imports::resource::<T, Endpoint>,
    )?;
    add_read(
        host,
        "[method]session-manager.build-session-context",
        Request::BuildContext,
        crate::read_imports::resource::<T, Endpoint>,
    )?;
    add_read(
        host,
        "[method]session-manager.reset-leaf",
        Request::ResetLeaf,
        crate::read_imports::resource::<T, Endpoint>,
    )?;
    add_mutations(host)?;
    add_appends(host)
}

/// Forwards the multi-operand append operations without flattening optional metadata.
fn add_appends<T: WasiView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap(
        "[method]session-manager.append-model-change",
        |mut store: StoreContextMut<'_, T>,
         (resource, provider, id): (Resource<Endpoint>, String, String)| {
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            Ok((endpoint.read(|reply| Request::Model(provider, id, reply)),))
        },
    )?;
    for (method, request) in [
        ("append-custom-entry", Request::Custom as fn(_, _, _) -> _),
        ("append-label-change", Request::Label),
    ] {
        host.func_wrap(
            &format!("[method]session-manager.{method}"),
            move |mut store: StoreContextMut<'_, T>,
                  (resource, id, value): (Resource<Endpoint>, String, Option<String>)| {
                let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
                Ok((endpoint.read(|reply| request(id, value, reply)),))
            },
        )?;
    }
    host.func_wrap(
        "[method]session-manager.append-compaction",
        |mut store: StoreContextMut<'_, T>,
         (resource, value, from_hook): (Resource<Endpoint>, String, Option<bool>)| {
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            Ok((endpoint.read(|reply| Request::Compaction(value, from_hook, reply)),))
        },
    )?;
    host.func_wrap(
        "[method]session-manager.branch-with-summary",
        |mut store: StoreContextMut<'_, T>, (resource, id, summary, metadata): SummaryArgs| {
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            Ok((endpoint.read(|reply| Request::Summary(id, summary, metadata, reply)),))
        },
    )?;
    Ok(())
}

/// Registers exact single-operand writable-session mutations.
fn add_mutations<T: WasiView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    add_operand(
        host,
        "[method]session-manager.set-session-file",
        Request::SetFile,
        crate::read_imports::operand::<T, Endpoint, _>,
    )?;
    add_operand(
        host,
        "[method]session-manager.new-session",
        Request::NewSession,
        crate::read_imports::operand::<T, Endpoint, _>,
    )?;
    for (method, request) in [
        ("get-children", Request::Children as fn(_, _) -> _),
        ("append-message", Request::Message),
        ("append-thinking-level-change", Request::Thinking),
        ("append-session-info", Request::Info),
        ("append-custom-message-entry", Request::CustomMessage),
    ] {
        add_operand(
            host,
            &format!("[method]session-manager.{method}"),
            request,
            crate::read_imports::operand::<T, Endpoint, _>,
        )?;
    }
    add_operand(
        host,
        "[method]session-manager.branch",
        Request::Branch,
        crate::read_imports::operand::<T, Endpoint, _>,
    )?;
    add_operand(
        host,
        "[method]session-manager.create-branched-session",
        Request::CreateBranched,
        crate::read_imports::operand::<T, Endpoint, _>,
    )?;
    Ok(())
}
