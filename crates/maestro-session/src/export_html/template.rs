//! Pure session-tree projection, content search and raw ancestry.
mod content;
pub use content::{extract_content, extract_content_prefix, has_text_content, read_text};
mod forest;
pub use forest::build_tree;
mod nodes;
mod search;
pub use search::{get_searchable_text, matches_search};
mod structure;
use crate::session_manager::{SessionEntry, SessionTreeNode as InputNode, context::ancestor_path};
pub use nodes::{SessionTreeNode, retain};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    hash::BuildHasher,
    io,
};
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

/// Project a row without frontend folding or theme policy.
#[must_use]
pub fn build_tree_prefix(row: &FlatNode, multiple_roots: bool) -> String {
    let indent = row.indent.saturating_sub(usize::from(multiple_roots));
    let connector = row.show_connector && !row.virtual_child;
    (0..indent)
        .map(|level| {
            if let Some(gutter) = row.gutters.iter().find(|g| g.position == level) {
                if gutter.show { "│  " } else { "   " }
            } else if connector && level + 1 == indent {
                if row.is_last { "└─ " } else { "├─ " }
            } else {
                "   "
            }
        })
        .collect()
}

/// Root-first original entries selected by the caller's lookup, stopping before repetition.
///
/// # Errors
/// Propagates a native JSON error for a reached null record.
pub fn get_path<'a>(
    target: Option<&str>,
    lookup: impl Fn(&str) -> Option<&'a SessionEntry>,
) -> Result<Vec<&'a SessionEntry>, serde_json::Error> {
    target
        .and_then(&lookup)
        .map_or_else(|| Ok(Vec::new()), |leaf| ancestor_path(leaf, lookup))
}
/// Owned identities on the raw-record path.
///
/// # Errors
/// Propagates errors from [`get_path`].
pub fn build_active_path_ids<'a>(
    target: Option<&str>,
    lookup: impl Fn(&str) -> Option<&'a SessionEntry>,
) -> Result<HashSet<String>, serde_json::Error> {
    Ok(get_path(target, lookup)?
        .into_iter()
        .filter_map(|entry| entry["id"].as_str().map(str::to_owned))
        .collect())
}
