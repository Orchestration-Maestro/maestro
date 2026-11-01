//! Independently retained nodes of one domain-produced session tree.
use crate::{
    history_reads::optional_string,
    local_resources::{Lease, Registry},
};
use maestro_extensions::SessionTreeNode;
use serde_json::Value;
use std::{
    rc::Rc,
    sync::{Arc, mpsc},
};

/// Catchable node results sent to the dedicated Store thread.
type Reply<T> = mpsc::Sender<Result<T, String>>;

/// Read-only operations on a captured node, not a fresh history traversal.
pub(crate) enum Request {
    /// Encodes the retained entry as an owned crossing value.
    Entry(Reply<String>),
    /// Retains each captured child independently.
    Children(Reply<Vec<Endpoint>>),
    /// Reads the label captured when the tree was built.
    Label(Reply<Option<String>>),
    /// Reads the timestamp captured when the tree was built.
    LabelTimestamp(Reply<Option<String>>),
}

impl Request {
    /// Settles requests after a capability ends.
    pub(crate) fn reject(self, error: &str) {
        match self {
            Self::Entry(reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::Children(reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::Label(reply) | Self::LabelTimestamp(reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
        }
    }
}

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

    /// Answers only from this captured tree, without borrowing its original reader.
    pub(crate) fn answer(&self, request: Request, registry: &Registry) {
        let node = &self.nodes[self.index];
        match request {
            Request::Entry(reply) => {
                let _ = reply
                    .send(serde_json::to_string(&node.entry).map_err(|error| error.to_string()));
            }
            Request::Children(reply) => {
                let result = node
                    .children
                    .iter()
                    .map(|&index| {
                        registry.node(Self {
                            nodes: Rc::clone(&self.nodes),
                            index,
                        })
                    })
                    .collect();
                let _ = reply.send(result);
            }
            Request::Label(reply) => {
                let _ = reply.send(Ok(node.label.clone()));
            }
            Request::LabelTimestamp(reply) => {
                let _ = reply.send(Ok(node.label_timestamp.clone()));
            }
        }
    }
}

/// A Store-owned node identity retaining its local captured tree.
#[derive(Clone)]
pub(crate) struct Endpoint(pub(crate) Arc<Lease>);
