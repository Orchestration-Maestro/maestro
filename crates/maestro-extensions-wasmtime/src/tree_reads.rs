//! Independently retained nodes of one domain-produced session tree.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    history_reads::optional_string,
    local_resources::{Lease, Registry, held::Node as Held},
};
use maestro_extensions::SessionTreeNode;
use serde_json::Value;
use std::{rc::Rc, sync::Arc};

/// One immutable captured record with flat child indexes.
struct Node {
    /// The native entry has no mutation operation through this read-only capability.
    entry: Value,
    /// Captured child ordering from the domain tree builder.
    children: Vec<usize>,
    /// Captured resolved label.
    label: Option<String>,
    /// Captured resolved label timestamp.
    label_timestamp: Option<String>,
}

/// Independent ownership of one node in a flat, non-recursively dropped tree.
#[derive(Clone)]
pub(crate) struct Owner {
    /// One allocation shared by independently retained child capabilities.
    nodes: Rc<Vec<Node>>,
    /// The captured position in that allocation.
    index: usize,
}

impl Owner {
    /// Owns the returned tree's immutable values beyond the native read borrow.
    pub(crate) fn capture(roots: Vec<SessionTreeNode<'_>>) -> Result<Vec<Self>, String> {
        let mut nodes: Vec<Node> = Vec::new();
        let mut root_indexes = Vec::new();
        let mut pending: Vec<(_, Option<usize>)> =
            roots.into_iter().rev().map(|node| (node, None)).collect();
        while let Some((mut node, parent)) = pending.pop() {
            let index = nodes.len();
            if let Some(parent) = parent {
                nodes[parent].children.push(index);
            } else {
                root_indexes.push(index);
            }
            nodes.push(Node {
                entry: node.entry.clone(),
                children: Vec::new(),
                label: optional_string(node.label)?,
                label_timestamp: optional_string(node.label_timestamp)?,
            });
            pending.extend(
                std::mem::take(&mut node.children)
                    .into_iter()
                    .rev()
                    .map(|node| (node, Some(index))),
            );
        }
        let nodes = Rc::new(nodes);
        Ok(root_indexes
            .into_iter()
            .map(|index| Self {
                nodes: Rc::clone(&nodes),
                index,
            })
            .collect())
    }

    /// Retains each captured child independently of this node's reader.
    fn children(&self, registry: &Registry) -> Result<Vec<Endpoint>, String> {
        self.nodes[self.index]
            .children
            .iter()
            .map(|&index| {
                registry.node(Self {
                    nodes: Rc::clone(&self.nodes),
                    index,
                })
            })
            .collect()
    }
}

/// A Store-owned node identity retaining its local captured tree.
#[derive(Clone)]
pub struct Endpoint(pub(crate) Arc<Lease>);

impl Endpoint {
    /// Encodes the retained entry as an owned crossing value.
    pub(crate) fn entry(&self) -> Result<String, String> {
        self.0.run::<Held, _>(|node, _| {
            serde_json::to_string(&node.nodes[node.index].entry).map_err(|error| error.to_string())
        })
    }

    /// Retains each captured child independently.
    pub(crate) fn children(&self) -> Result<Vec<Endpoint>, String> {
        self.0.run::<Held, _>(Owner::children)
    }

    /// Reads the label captured when the tree was built.
    pub(crate) fn label(&self) -> Result<Option<String>, String> {
        self.0
            .run::<Held, _>(|node, _| Ok(node.nodes[node.index].label.clone()))
    }

    /// Reads the timestamp captured when the tree was built.
    pub(crate) fn label_timestamp(&self) -> Result<Option<String>, String> {
        self.0
            .run::<Held, _>(|node, _| Ok(node.nodes[node.index].label_timestamp.clone()))
    }
}
