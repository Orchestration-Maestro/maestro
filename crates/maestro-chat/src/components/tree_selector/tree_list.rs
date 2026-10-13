//! Borrowed tree ordering, entry filtering and selection.
use super::FilterMode;
use maestro_app::{SessionTreeNode, is_truthy, is_whitespace_scalar};
use serde_json::Value;
use std::collections::HashMap;
use std::io;

/// A selection callback accepting the retained entry identity.
type SelectCallback<'tree> = Box<dyn FnMut(&str) + 'tree>;

/// A navigation view borrowing the supplied session tree.
pub struct TreeList<'tree, 'entry: 'tree> {
    /// Callback receiving the confirmed entry identity.
    pub on_select: Option<SelectCallback<'tree>>,
    /// Callback for ordinary cancellation.
    pub on_cancel: Option<Box<dyn FnMut() + 'tree>>,
    /// Nodes in navigation order.
    nodes: Vec<&'tree SessionTreeNode<'entry>>,
    /// Last occurrence of each entry identity.
    parents: HashMap<&'tree str, &'tree SessionTreeNode<'entry>>,
    /// Nodes admitted by the current filter.
    pub(super) visible: Vec<&'tree SessionTreeNode<'entry>>,
    /// Selected position in the visible view.
    pub(super) selected: usize,
    /// Entry categories admitted by the view.
    pub(super) mode: FilterMode,
    /// Page movement distance.
    pub(super) max_visible_lines: usize,
    /// Current history leaf identity.
    leaf: Option<&'tree str>,
    /// Last selection retained through empty views.
    bookmark: Option<&'tree str>,
}
impl<'tree, 'entry: 'tree> TreeList<'tree, 'entry> {
    /// Orders and filters a borrowed tree, selecting the explicit target or leaf.
    ///
    /// # Errors
    /// Returns invalid data when a reached message or content read is malformed.
    pub fn new(
        tree: &'tree [SessionTreeNode<'entry>],
        current_leaf_id: Option<&'tree str>,
        max_visible_lines: usize,
        initial_selected_id: Option<&str>,
        initial_filter_mode: Option<FilterMode>,
    ) -> io::Result<Self> {
        let nodes = flatten_tree(tree, current_leaf_id)?;
        let parents = nodes
            .iter()
            .filter_map(|node| node.entry.get("id")?.as_str().map(|id| (id, *node)))
            .collect();
        let mut list = Self {
            on_select: None,
            on_cancel: None,
            nodes,
            parents,
            visible: Vec::new(),
            selected: 0,
            mode: initial_filter_mode.unwrap_or_default(),
            max_visible_lines,
            leaf: current_leaf_id,
            bookmark: None,
        };
        list.apply_filter()?;
        list.selected = list.nearest_visible(initial_selected_id.or(current_leaf_id));
        list.remember_selection();
        Ok(list)
    }

    /// Borrows the selected original node, or returns none for an empty view.
    #[must_use]
    pub fn get_selected_node(&self) -> Option<&'tree SessionTreeNode<'entry>> {
        self.visible.get(self.selected).copied()
    }

    /// Selects the last matching occurrence, walking stored parent identities.
    fn nearest_visible<'target>(&'target self, mut target: Option<&'target str>) -> usize {
        let indices: HashMap<_, _> = self
            .visible
            .iter()
            .enumerate()
            .filter_map(|(i, node)| node.entry.get("id")?.as_str().map(|id| (id, i)))
            .collect();
        while let Some(id) = target {
            if let Some(index) = indices.get(id) {
                return *index;
            }
            target = self
                .parents
                .get(id)
                .and_then(|node| node.entry.get("parentId"))
                .and_then(serde_json::Value::as_str);
        }
        self.visible.len().saturating_sub(1)
    }

    /// Rebuilds the view and keeps the last valid selection through empty views.
    pub(super) fn apply_filter(&mut self) -> io::Result<()> {
        let mut visible = Vec::new();
        for node in &self.nodes {
            if self.includes(node)? {
                visible.push(*node);
            }
        }
        self.visible = visible;
        self.selected = self.nearest_visible(self.bookmark);
        self.remember_selection();
        Ok(())
    }

    /// Checks assistant eligibility before applying the selected entry categories.
    fn includes(&self, node: &SessionTreeNode<'_>) -> io::Result<bool> {
        let entry = node.entry;
        let message = if entry["type"] == "message" {
            Some(read_message(entry)?)
        } else {
            None
        };
        let role = message
            .and_then(|value| value.get("role"))
            .and_then(Value::as_str);
        if role == Some("assistant") && !self.leaf.is_some_and(|id| entry["id"] == id) {
            let message = read_message(entry)?;
            let text = has_text_content(&message["content"])?;
            let stop = &message["stopReason"];
            if !(text || is_truthy(stop) && stop != "stop" && stop != "toolUse") {
                return Ok(false);
            }
        }
        let settings = matches!(
            entry["type"].as_str(),
            Some("label" | "custom" | "model_change" | "thinking_level_change" | "session_info")
        );
        Ok(match self.mode {
            FilterMode::Default => !settings,
            FilterMode::NoTools => !settings && role != Some("toolResult"),
            FilterMode::UserOnly => role == Some("user"),
            FilterMode::LabeledOnly => node.label.is_some(),
            FilterMode::All => true,
        })
    }

    /// Updates the bookmark only when a node is selected.
    pub(super) fn remember_selection(&mut self) {
        if let Some(node) = self.get_selected_node() {
            self.bookmark = node.entry.get("id").and_then(serde_json::Value::as_str);
        }
    }
}

/// Computes active subtree membership once, then walks stable active-first groups.
fn flatten_tree<'tree, 'entry>(
    tree: &'tree [SessionTreeNode<'entry>],
    leaf: Option<&str>,
) -> io::Result<Vec<&'tree SessionTreeNode<'entry>>> {
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
    nodes: &'tree [SessionTreeNode<'entry>],
    active: &HashMap<*const SessionTreeNode<'entry>, bool>,
    pending: &mut Vec<&'tree SessionTreeNode<'entry>>,
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
fn read_message(entry: &Value) -> io::Result<&Value> {
    entry
        .get("message")
        .filter(|message| !message.is_null())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing message receiver"))
}

/// Tests string and array content, stopping before unread later malformed text.
fn has_text_content(content: &Value) -> io::Result<bool> {
    if let Some(text) = content.as_str() {
        return Ok(text.chars().any(|scalar| !is_whitespace_scalar(scalar)));
    }
    if let Some(blocks) = content.as_array() {
        for block in blocks
            .iter()
            .filter(|block| block.get("type").is_some_and(|kind| kind == "text"))
        {
            let text = &block["text"];
            if !is_truthy(text) {
                continue;
            }
            let text = text.as_str().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "text block is not a string")
            })?;
            if text.chars().any(|scalar| !is_whitespace_scalar(scalar)) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
