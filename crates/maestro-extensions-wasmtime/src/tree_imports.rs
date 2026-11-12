//! Canonical tree-node methods over the captured tree the local owner retains.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    bindings::maestro::extension::host::HostSessionTreeNode,
    lifecycle::State,
    read_imports::{endpoint, forwards, release},
    tree_reads::Endpoint,
};
use wasmtime::component::{LinkerInstance, Resource};
use wasmtime_wasi::WasiView;

impl HostSessionTreeNode for State {
    fn entry(&mut self, this: Resource<Endpoint>) -> wasmtime::Result<Result<String, String>> {
        Ok(endpoint(self, &this)?.entry())
    }

    fn children(
        &mut self,
        this: Resource<Endpoint>,
    ) -> wasmtime::Result<Result<Vec<Resource<Endpoint>>, String>> {
        let children = endpoint(self, &this)?.children();
        lower_nodes(self, children)
    }

    fn label(
        &mut self,
        this: Resource<Endpoint>,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        Ok(endpoint(self, &this)?.label())
    }

    fn label_timestamp(
        &mut self,
        this: Resource<Endpoint>,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        Ok(endpoint(self, &this)?.label_timestamp())
    }

    fn drop(&mut self, this: Resource<Endpoint>) -> wasmtime::Result<()> {
        release(self, this)
    }
}

/// Defines the captured node methods as forwards to the trait, with actual typed destruction.
pub(crate) fn add_to_linker(host: &mut LinkerInstance<'_, State>) -> wasmtime::Result<()> {
    crate::context_imports::add_resource::<State, Endpoint>(host, "session-tree-node")?;
    forwards! { host, "session-tree-node", HostSessionTreeNode, Endpoint,
        "entry" => entry();
        "label" => label();
        "label-timestamp" => label_timestamp();
        "children" => children();
    }
    Ok(())
}

/// Transfers each independently retained node to the Store table without changing order.
pub(crate) fn lower_nodes(
    state: &mut State,
    nodes: Result<Vec<Endpoint>, String>,
) -> wasmtime::Result<Result<Vec<Resource<Endpoint>>, String>> {
    match nodes {
        Ok(nodes) => Ok(Ok(nodes
            .into_iter()
            .map(|node| state.ctx().table.push(node))
            .collect::<Result<Vec<_>, _>>()?)),
        Err(error) => Ok(Err(error)),
    }
}
