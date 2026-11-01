//! Canonical tree-node resources retaining their original captured native tree.
use crate::read_imports::ReadEndpoint;
use crate::read_imports::add_read;
use crate::{
    context_imports::add_resource,
    tree_reads::{Endpoint, Request},
};
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};
use wasmtime_wasi::WasiView;

/// Defines the captured node methods with actual typed resource destruction.
pub(crate) fn add_to_linker<T: WasiView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    add_resource::<T, Endpoint>(host, "session-tree-node")?;
    add_read(
        host,
        "[method]session-tree-node.entry",
        Request::Entry,
        crate::read_imports::resource::<T, Endpoint>,
    )?;
    add_read(
        host,
        "[method]session-tree-node.label",
        Request::Label,
        crate::read_imports::resource::<T, Endpoint>,
    )?;
    add_read(
        host,
        "[method]session-tree-node.label-timestamp",
        Request::LabelTimestamp,
        crate::read_imports::resource::<T, Endpoint>,
    )?;
    host.func_wrap(
        "[method]session-tree-node.children",
        |mut store: StoreContextMut<'_, T>, (resource,): (Resource<Endpoint>,)| {
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            let result = lower_nodes(&mut store, endpoint.read(Request::Children))?;
            Ok((result,))
        },
    )?;
    Ok(())
}

/// Transfers each independently retained node to the Store table without changing order.
pub(crate) fn lower_nodes<T: WasiView>(
    store: &mut StoreContextMut<'_, T>,
    nodes: Result<Vec<Endpoint>, String>,
) -> wasmtime::Result<Result<Vec<Resource<Endpoint>>, String>> {
    match nodes {
        Ok(nodes) => Ok(Ok(nodes
            .into_iter()
            .map(|node| store.data_mut().ctx().table.push(node))
            .collect::<Result<Vec<_>, _>>()?)),
        Err(error) => Ok(Err(error)),
    }
}
