//! Canonical tree-node methods over the captured tree the local owner retains.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    bindings::maestro::extension::host::HostSessionTreeNode, lifecycle::State,
    read_imports::forward, tree_reads::Endpoint,
};
use wasmtime::component::{LinkerInstance, Resource};
use wasmtime_wasi::WasiView;

/// Clones the endpoint a guest handle names before native code runs.
fn endpoint(state: &mut State, this: &Resource<Endpoint>) -> wasmtime::Result<Endpoint> {
    Ok(state.ctx().table.get(this)?.clone())
}

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
        self.ctx().table.delete(this)?;
        Ok(())
    }
}

/// Defines the captured node methods as forwards to the trait, with actual typed destruction.
pub(crate) fn add_to_linker(host: &mut LinkerInstance<'_, State>) -> wasmtime::Result<()> {
    crate::context_imports::add_resource::<State, Endpoint>(host, "session-tree-node")?;
    forward(host, "[method]session-tree-node.entry", |state, (this,)| {
        HostSessionTreeNode::entry(state, this)
    })?;
    forward(host, "[method]session-tree-node.label", |state, (this,)| {
        HostSessionTreeNode::label(state, this)
    })?;
    forward(
        host,
        "[method]session-tree-node.label-timestamp",
        |state, (this,)| HostSessionTreeNode::label_timestamp(state, this),
    )?;
    forward(
        host,
        "[method]session-tree-node.children",
        |state, (this,)| HostSessionTreeNode::children(state, this),
    )
}

/// Transfers each independently retained node to the Store table without changing order.
pub(crate) fn lower_nodes<T: WasiView>(
    state: &mut T,
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
