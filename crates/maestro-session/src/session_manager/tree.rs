#![doc = include_str!("../../../../docs/tree.md")]
use super::{
    FileEntry, LabelEntry, ReadonlySessionManager, SessionEntry, SessionManager, SessionTreeNode,
    context::{self, ecma_whitespace, truthy},
    conversions::invalid,
};
use maestro_request::presence::Presence;
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::io;

/// Apply a label record to the resolved maps; a falsy or absent label clears its target.
///
/// Targets that are not strings can never match an entry identity and are ignored.
pub(super) fn apply_label(
    labels: &mut Map<String, Value>,
    timestamps: &mut Map<String, Value>,
    entry: &FileEntry,
) {
    let Some(target) = entry["targetId"].as_str() else {
        return;
    };
    let label = entry.get("label").filter(|label| truthy(label));
    let stamp = entry.get("timestamp").filter(|_| label.is_some());
    for (map, value) in [(labels, label), (timestamps, stamp)] {
        match value {
            Some(value) => map.insert(target.to_owned(), value.clone()),
            None => map.shift_remove(target),
        };
    }
}

impl ReadonlySessionManager for SessionManager {
    fn get_cwd(&self) -> &str {
        &self.cwd
    }
    fn get_session_dir(&self) -> &str {
        &self.session_dir
    }
    fn get_session_id(&self) -> &str {
        &self.session_id
    }
    fn get_session_file(&self) -> Option<&str> {
        self.session_file.as_deref()
    }
    fn get_leaf_id(&self) -> Option<&str> {
        self.leaf_id.as_deref()
    }
    fn get_leaf_entry(&self) -> Option<&SessionEntry> {
        self.get_leaf_id().and_then(|id| self.get_entry(id))
    }
    fn get_entry(&self, id: &str) -> Option<&SessionEntry> {
        self.by_id
            .get(id)
            .and_then(|position| self.file_entries.get(*position))
    }
    fn get_label(&self, id: &str) -> Option<&Value> {
        self.labels.get(id)
    }
    fn get_branch(&self, from_id: Option<&str>) -> Result<Vec<&SessionEntry>, serde_json::Error> {
        match from_id
            .or(self.get_leaf_id())
            .and_then(|id| self.get_entry(id))
        {
            Some(leaf) => context::ancestor_path(leaf, |id| self.get_entry(id)),
            None => Ok(Vec::new()),
        }
    }
    fn get_header(&self) -> Option<&FileEntry> {
        self.file_entries.first()
    }
    fn get_entries(&self) -> Vec<&SessionEntry> {
        self.file_entries
            .iter()
            .filter(|entry| entry["type"] != "session")
            .collect()
    }
    fn get_tree(&self) -> Vec<SessionTreeNode<'_>> {
        let shape = Shape::new(self);
        let mut active = vec![false; shape.nodes.len()];
        shape
            .roots
            .iter()
            .filter_map(|&root| self.expand(&shape, root, &mut active))
            .collect()
    }
    fn get_session_name(&self) -> io::Result<Option<&str>> {
        let newest = self
            .file_entries
            .iter()
            .rev()
            .find(|entry| entry["type"] == "session_info");
        match newest.map(|entry| &entry["name"]) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(name)) => {
                Ok(Some(name.trim_matches(ecma_whitespace)).filter(|name| !name.is_empty()))
            }
            Some(_) => Err(invalid("nonstring session name")),
        }
    }
}

impl SessionManager {
    /// Borrow direct children of an identity in first-seen identity order.
    ///
    /// Each identity contributes its last record, whose own parent decides membership.
    #[must_use]
    pub fn get_children(&self, parent_id: &str) -> Vec<&SessionEntry> {
        let mut seen = HashSet::new();
        self.get_entries()
            .into_iter()
            .filter_map(|entry| entry["id"].as_str())
            .filter(|id| seen.insert(*id))
            .filter_map(|id| self.get_entry(id))
            .filter(|entry| entry["parentId"].as_str() == Some(parent_id))
            .collect()
    }

    /// Select the position before all entries without writing anything.
    pub fn reset_leaf(&mut self) {
        self.leaf_id = None;
    }

    /// Append a label change below the selected entry, updating the resolved label maps
    /// only after the publication routine returns successfully (it may complete without writing).
    ///
    /// A missing, null or empty label clears the target's label.
    ///
    /// # Errors
    /// Returns `Entry {id} not found` before any change when the target is not indexed;
    /// a publication error leaves the accepted entry but the previous resolved labels.
    pub fn append_label_change(
        &mut self,
        target_id: &str,
        label: Presence<&str>,
    ) -> io::Result<String> {
        self.require_entry(target_id)?;
        let id = self.append_record(|base| LabelEntry {
            base,
            target_id: target_id.to_owned(),
            label: match label {
                Presence::Missing => Presence::Missing,
                Presence::Null => Presence::Null,
                Presence::Present(text) => Presence::Present(text.to_owned()),
            },
        })?;
        if let Some(entry) = self.file_entries.last() {
            apply_label(&mut self.labels, &mut self.label_timestamps, entry);
        }
        Ok(id)
    }

    /// Build the borrowed subtree at a root, omitting only edges back into the active path.
    ///
    /// `active` marks the nodes on the path being built and is all false again on return.
    fn expand<'a>(
        &'a self,
        shape: &Shape<'a>,
        root: usize,
        active: &mut [bool],
    ) -> Option<SessionTreeNode<'a>> {
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
                None => self.close(shape, &mut stack, active),
            };
        }
        finished
    }

    /// Finish the innermost node, handing it to its parent frame or returning the root.
    fn close<'a>(
        &'a self,
        shape: &Shape<'a>,
        stack: &mut Vec<Frame<'a>>,
        active: &mut [bool],
    ) -> Option<SessionTreeNode<'a>> {
        let frame = stack.pop()?;
        active[frame.node] = false;
        let entry = shape.nodes[frame.node];
        let id = entry["id"].as_str().unwrap_or_default();
        let node = SessionTreeNode {
            entry,
            children: frame.built,
            label: self.labels.get(id),
            label_timestamp: self.label_timestamps.get(id),
        };
        match stack.last_mut() {
            Some(parent) => {
                parent.built.push(node);
                None
            }
            None => Some(node),
        }
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
    fn new(manager: &'a SessionManager) -> Self {
        let records = manager.get_entries();
        let mut index: HashMap<&str, usize> = HashMap::new();
        let mut nodes = Vec::new();
        for id in records.iter().filter_map(|record| record["id"].as_str()) {
            if let (None, Some(entry)) = (index.get(id), manager.get_entry(id)) {
                index.insert(id, nodes.len());
                nodes.push(entry);
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
