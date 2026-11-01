//! Canonical reader imports forwarding to independently retained local history.
use crate::read_imports::ReadEndpoint;
use crate::read_imports::{add_operand, add_read};
use crate::{
    context_imports::add_resource,
    history_reads::{Endpoint, Request},
};
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};
use wasmtime_wasi::WasiView;

/// Defines live reader operations and independently retained captured tree nodes.
pub(crate) fn add_to_linker<T: WasiView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    add_resource::<T, Endpoint>(host, "readonly-session-manager")?;
    host.func_wrap(
        "[method]readonly-session-manager.get-tree",
        |mut store: StoreContextMut<'_, T>, (resource,): (Resource<Endpoint>,)| {
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            let result =
                crate::tree_imports::lower_nodes(&mut store, endpoint.read(Request::Tree))?;
            Ok((result,))
        },
    )?;
    add_properties(host)?;
    add_operand(
        host,
        "[method]readonly-session-manager.get-entry",
        Request::Entry,
        crate::read_imports::operand::<T, Endpoint, _>,
    )?;
    add_operand(
        host,
        "[method]readonly-session-manager.get-label",
        Request::Label,
        crate::read_imports::operand::<T, Endpoint, _>,
    )?;
    add_operand(
        host,
        "[method]readonly-session-manager.get-branch",
        Request::Branch,
        crate::read_imports::operand::<T, Endpoint, _>,
    )
}

/// Registers the original reader's no-operand identity and record properties.
fn add_properties<T: WasiView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    add_read(
        host,
        "[method]readonly-session-manager.get-cwd",
        Request::Cwd,
        crate::read_imports::resource::<T, Endpoint>,
    )?;
    add_read(
        host,
        "[method]readonly-session-manager.get-session-dir",
        Request::SessionDir,
        crate::read_imports::resource::<T, Endpoint>,
    )?;
    add_read(
        host,
        "[method]readonly-session-manager.get-session-id",
        Request::SessionId,
        crate::read_imports::resource::<T, Endpoint>,
    )?;
    add_read(
        host,
        "[method]readonly-session-manager.get-session-file",
        Request::SessionFile,
        crate::read_imports::resource::<T, Endpoint>,
    )?;
    add_read(
        host,
        "[method]readonly-session-manager.get-leaf-id",
        Request::LeafId,
        crate::read_imports::resource::<T, Endpoint>,
    )?;
    add_read(
        host,
        "[method]readonly-session-manager.get-leaf-entry",
        Request::LeafEntry,
        crate::read_imports::resource::<T, Endpoint>,
    )?;
    add_read(
        host,
        "[method]readonly-session-manager.get-header",
        Request::Header,
        crate::read_imports::resource::<T, Endpoint>,
    )?;
    add_read(
        host,
        "[method]readonly-session-manager.get-entries",
        Request::Entries,
        crate::read_imports::resource::<T, Endpoint>,
    )?;
    add_read(
        host,
        "[method]readonly-session-manager.get-session-name",
        Request::SessionName,
        crate::read_imports::resource::<T, Endpoint>,
    )?;
    Ok(())
}
