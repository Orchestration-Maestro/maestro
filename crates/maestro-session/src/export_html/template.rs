//! Pure session-tree ordering and geometry.
mod nodes;
mod structure;
use crate::session_manager::SessionTreeNode as InputNode;
pub use nodes::{SessionTreeNode, retain};
use serde_json::Value;
use std::{collections::HashMap, hash::BuildHasher, io};
pub use structure::{FlatNode, GutterInfo, Structure, physical, recalculate_visual_structure};
/// Computes active subtree membership once, then walks stable active-first groups.
///
/// # Errors
/// Returns invalid data for a reached missing or null message receiver.
pub fn flatten_tree<'tree, 'entry>(
    tree: &'tree [InputNode<'entry>],
    leaf: Option<&str>,
) -> io::Result<Vec<&'tree InputNode<'entry>>> {
    let mut preorder = Vec::new();
    let mut pending: Vec<_> = tree.iter().collect();
    while let Some(node) = pending.pop() {
        preorder.push(node);
        pending.extend(&node.children);
    }
    let mut active = HashMap::new();
    for node in preorder.into_iter().rev() {
        let contains = leaf.is_some_and(|id| node.entry["id"] == id)
            || node
                .children
                .iter()
                .any(|child| active[&std::ptr::from_ref(child)]);
        active.insert(std::ptr::from_ref(node), contains);
    }
    push_ordered(tree, &active, &mut pending);
    let mut ordered = Vec::new();
    while let Some(node) = pending.pop() {
        if node.entry["type"] == "message" {
            read_message(node.entry)?;
        }
        ordered.push(node);
        push_ordered(&node.children, &active, &mut pending);
    }
    Ok(ordered)
}

/// Pushes reversed inactive and active groups for active-first stack traversal.
fn push_ordered<'tree, 'entry>(
    nodes: &'tree [InputNode<'entry>],
    active: &HashMap<*const InputNode<'entry>, bool>,
    pending: &mut Vec<&'tree InputNode<'entry>>,
) {
    for group in [false, true] {
        pending.extend(
            nodes
                .iter()
                .rev()
                .filter(|node| active[&std::ptr::from_ref(*node)] == group),
        );
    }
}

/// Reads the message receiver without eagerly decoding its fields.
///
/// # Errors
/// Returns invalid data for a missing or null message receiver.
pub fn read_message(entry: &Value) -> io::Result<&Value> {
    entry
        .get("message")
        .filter(|message| !message.is_null())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing message receiver"))
}
/// Walks target-first stored parents, bounded by the retained identity count.
pub fn ancestors<'a, S: BuildHasher>(
    parents: &'a HashMap<String, SessionTreeNode, S>,
    target: Option<&'a str>,
) -> impl Iterator<Item = &'a str> {
    std::iter::successors(target, |id| parents.get(*id)?.entry()["parentId"].as_str())
        .take(parents.len())
}
