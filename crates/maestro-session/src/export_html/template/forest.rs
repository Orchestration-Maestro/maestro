//! Shared borrowed forest construction from caller-selected metadata.
use crate::session_manager::{SessionEntry, SessionTreeNode};
use serde_json::Value;
use std::collections::HashMap;

/// Build a forest borrowing each identity's last record and the supplied metadata.
///
/// Each string-identified record contributes an edge; active-path back edges are omitted.
/// Roots retain record order and children use the shared timestamp ordering.
pub fn build_tree<'a>(
    entries: impl IntoIterator<Item = &'a SessionEntry>,
    label: impl Fn(&str) -> Option<&'a Value>,
    label_timestamp: impl Fn(&str) -> Option<&'a Value>,
) -> Vec<SessionTreeNode<'a>> {
    let shape = Shape::new(entries.into_iter().collect());
    let mut active = vec![false; shape.nodes.len()];
    shape
        .roots
        .iter()
        .filter_map(|&root| expand(&shape, root, &mut active, &label, &label_timestamp))
        .collect()
}

/// Build the borrowed subtree at a root, omitting only edges back into the active path.
///
/// `active` marks the nodes on the path being built and is all false again on return.
fn expand<'entry>(
    shape: &Shape<'entry>,
    root: usize,
    active: &mut [bool],
    label: &impl Fn(&str) -> Option<&'entry Value>,
    label_timestamp: &impl Fn(&str) -> Option<&'entry Value>,
) -> Option<SessionTreeNode<'entry>> {
    active[root] = true;
    let mut stack = vec![Frame {
        node: root,
        ..Frame::default()
    }];
    let mut finished = None;
    while finished.is_none() {
        let top = stack.last_mut()?;
        let next = shape.children[top.node].get(top.next).copied();
        top.next += usize::from(next.is_some());
        finished = match next {
            Some(child) if !active[child] => {
                active[child] = true;
                stack.push(Frame {
                    node: child,
                    ..Frame::default()
                });
                None
            }
            Some(_) => None,
            None => close(shape, &mut stack, active, label, label_timestamp),
        };
    }
    finished
}

/// Finish the innermost node, handing it to its parent frame or returning the root.
fn close<'entry>(
    shape: &Shape<'entry>,
    stack: &mut Vec<Frame<'entry>>,
    active: &mut [bool],
    label: &impl Fn(&str) -> Option<&'entry Value>,
    label_timestamp: &impl Fn(&str) -> Option<&'entry Value>,
) -> Option<SessionTreeNode<'entry>> {
    let frame = stack.pop()?;
    active[frame.node] = false;
    let entry = shape.nodes[frame.node];
    let id = entry["id"].as_str().unwrap_or_default();
    let node = SessionTreeNode {
        entry,
        children: frame.built,
        label: label(id),
        label_timestamp: label_timestamp(id),
    };
    match stack.last_mut() {
        Some(parent) => {
            parent.built.push(node);
            None
        }
        None => Some(node),
    }
}

/// Unique identities with their parent-to-child edges, one per retained record.
struct Shape<'a> {
    /// Last record per identity, in first-seen order.
    nodes: Vec<&'a SessionEntry>,
    /// Node positions without a usable parent, in record order.
    roots: Vec<usize>,
    /// Child node positions per node, in chronological order.
    children: Vec<Vec<usize>>,
}

impl<'a> Shape<'a> {
    /// Index identities, attach every record to its parent and order each child list by
    /// parsed instant, with unparsable instants last and equal keys in record order.
    fn new(records: Vec<&'a SessionEntry>) -> Self {
        let mut index: HashMap<&str, usize> = HashMap::new();
        let mut nodes = Vec::new();
        for record in &records {
            let Some(id) = record["id"].as_str() else {
                continue;
            };
            if let Some(&position) = index.get(id) {
                nodes[position] = *record;
            } else {
                index.insert(id, nodes.len());
                nodes.push(*record);
            }
        }
        let mut roots = Vec::new();
        let mut children = vec![Vec::new(); nodes.len()];
        for record in records {
            let Some(id) = record["id"].as_str() else {
                continue;
            };
            let Some(&node) = index.get(id) else { continue };
            let parent = record["parentId"]
                .as_str()
                .filter(|parent| *parent != id)
                .and_then(|parent| index.get(parent));
            match parent {
                Some(&parent) => children[parent].push(node),
                None => roots.push(node),
            }
        }
        let times: Vec<f64> = nodes
            .iter()
            .map(|entry| {
                let instant = entry["timestamp"]
                    .as_str()
                    .map(crate::messages::timestamp_millis);
                instant
                    .filter(|time| !time.is_nan())
                    .unwrap_or(f64::INFINITY)
            })
            .collect();
        for list in &mut children {
            list.sort_by(|&a, &b| times[a].total_cmp(&times[b]));
        }
        Self {
            nodes,
            roots,
            children,
        }
    }
}

/// Progress through one node's children while its subtree is built.
#[derive(Default)]
struct Frame<'a> {
    /// Node position being built.
    node: usize,
    /// Next child edge to visit.
    next: usize,
    /// Completed child subtrees.
    built: Vec<SessionTreeNode<'a>>,
}
