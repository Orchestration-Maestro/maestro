//! Retained tree ordering, entry filtering and selection.
use super::navigation::Notification;
use super::{
    FilterMode, SessionTreeNode,
    structure::{FlatNode, Structure},
};
use maestro_app::export_html::template::{
    ancestors, flatten_tree, has_text_content, read_message, recalculate_visual_structure,
};
use maestro_app::{SessionTreeNode as InputNode, is_truthy};
use maestro_theme::LiveTheme;
use maestro_tui::tui::TuiRuntime;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::{cell::RefCell, io, rc::Rc};

/// Mutable navigation state, independently borrowed from notification slots.
#[derive(Clone)]
pub(super) struct TreeState {
    /// Nodes in navigation order.
    pub(super) nodes: Rc<Vec<SessionTreeNode>>,
    /// Last occurrence of each entry identity.
    pub(super) parents: Rc<HashMap<String, SessionTreeNode>>,
    /// Nodes admitted by the current filter.
    pub(super) visible: Vec<SessionTreeNode>,
    /// Selected position in the visible view.
    pub(super) selected: usize,
    /// Entry categories admitted by the view.
    pub(super) mode: FilterMode,
    /// Page movement distance.
    pub(super) max_visible_lines: usize,
    /// Current history leaf identity.
    pub(super) leaf: Option<String>,
    /// Physical occurrence geometry, retained across filtered projections.
    pub(super) physical: Rc<Vec<FlatNode>>,
    /// Current visible geometry and ancestry.
    pub(super) structure: Structure,
    /// Stored ancestry of the current history leaf.
    pub(super) active_path: Rc<HashSet<String>>,
    /// Last assistant call for each tool identity.
    pub(super) tool_calls: Rc<HashMap<String, (usize, usize)>>,
    /// Folded semantic entry identities.
    pub(super) folded: HashSet<String>,
    /// Authored literal search query.
    pub(super) query: String,
    /// Whether label times are displayed.
    pub(super) show_label_timestamps: bool,
    /// Wall-clock observation shared by rendering and updates.
    pub(super) now: Rc<dyn Fn() -> std::time::SystemTime>,
    /// Last selection retained through empty views.
    bookmark: Option<String>,
}
impl TreeState {
    /// Orders and filters a retained tree, selecting the explicit target or leaf.
    ///
    /// # Errors
    /// Returns invalid data when a reached message or content read is malformed.
    pub fn new(
        tree: &[InputNode<'_>],
        current_leaf_id: Option<&str>,
        max_visible_lines: usize,
        initial_selected_id: Option<&str>,
        initial_filter_mode: Option<FilterMode>,
    ) -> io::Result<Self> {
        let nodes = Rc::new(super::nodes::retain(&flatten_tree(tree, current_leaf_id)?));
        let parents = nodes
            .iter()
            .filter_map(|node| {
                node.entry()
                    .get("id")?
                    .as_str()
                    .map(|id| (id.to_owned(), node.clone()))
            })
            .collect();
        let mut list = Self {
            active_path: Rc::new(HashSet::new()),
            physical: Rc::new(super::structure::physical(&nodes)),
            tool_calls: Rc::new(super::tool_call::collect(&nodes)),
            structure: Structure::default(),
            nodes,
            parents: Rc::new(parents),
            visible: Vec::new(),
            selected: 0,
            mode: initial_filter_mode.unwrap_or_default(),
            max_visible_lines,
            leaf: current_leaf_id.map(str::to_owned),
            bookmark: None,
            query: String::new(),
            show_label_timestamps: false,
            now: Rc::new(std::time::SystemTime::now),
            folded: HashSet::new(),
        };
        list.active_path = Rc::new(
            ancestors(&list.parents, current_leaf_id)
                .map(str::to_owned)
                .collect(),
        );
        list.apply_filter()?;
        list.selected = list.nearest_visible(initial_selected_id.or(current_leaf_id));
        list.remember_selection();
        Ok(list)
    }

    /// Retains the selected occurrence, or returns none for an empty view.
    #[must_use]
    pub fn get_selected_node(&self) -> Option<SessionTreeNode> {
        self.visible.get(self.selected).cloned()
    }

    /// Selects the last visible match, walking parents from the last full-tree match.
    fn nearest_visible<'target>(&'target self, target: Option<&'target str>) -> usize {
        if self.visible.is_empty() {
            return 0;
        }
        let indices: HashMap<_, _> = self
            .visible
            .iter()
            .enumerate()
            .filter_map(|(i, node)| node.entry().get("id")?.as_str().map(|id| (id, i)))
            .collect();
        for id in ancestors(&self.parents, target) {
            if let Some(index) = indices.get(id) {
                return *index;
            }
        }
        self.visible.len().saturating_sub(1)
    }

    /// Rebuilds the view and keeps the last valid selection through empty views.
    pub(super) fn apply_filter(&mut self) -> io::Result<()> {
        let mut visible = Vec::new();
        for node in self.nodes.iter() {
            if self.includes(node)? && super::search::matches(node, &self.query)? {
                visible.push(node.clone());
            }
        }
        let mut hidden = HashSet::new();
        for node in self.nodes.iter() {
            if let Some(parent) = node.entry()["parentId"].as_str()
                && (self.folded.contains(parent) || hidden.contains(parent))
                && let Some(id) = node.entry()["id"].as_str()
            {
                hidden.insert(id.to_owned());
            }
        }
        visible.retain(|node| {
            node.entry()["id"]
                .as_str()
                .is_none_or(|id| !hidden.contains(id))
        });
        self.visible = visible;
        self.structure =
            recalculate_visual_structure(&self.nodes, &self.visible, &self.physical, &self.parents);
        self.selected = self.nearest_visible(self.bookmark.as_deref());
        self.remember_selection();
        Ok(())
    }

    /// Checks assistant eligibility before applying the selected entry categories.
    fn includes(&self, node: &SessionTreeNode) -> io::Result<bool> {
        let entry = node.entry();
        let message = if entry["type"] == "message" {
            Some(read_message(entry)?)
        } else {
            None
        };
        let role = message
            .and_then(|value| value.get("role"))
            .and_then(Value::as_str);
        if role == Some("assistant") && !self.leaf.as_deref().is_some_and(|id| entry["id"] == id) {
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
            FilterMode::LabeledOnly => node.label().is_some(),
            FilterMode::All => true,
        })
    }

    /// Updates the bookmark only when a node is selected.
    pub(super) fn remember_selection(&mut self) {
        if let Some(node) = self.get_selected_node() {
            self.bookmark = node
                .entry()
                .get("id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned);
        }
    }
}

/// A shared selection notification.
type SelectCallback = Rc<dyn Fn(&str)>;
/// A shared label-edit notification.
type LabelCallback = Rc<dyn Fn(&str, Option<Value>)>;

/// A retained session-tree navigation view with replaceable notifications.
pub struct TreeList {
    /// Copy-on-write navigation snapshot, allowing render-time reentry.
    state: RefCell<Rc<TreeState>>,
    /// Current confirmation notification.
    select: RefCell<Option<SelectCallback>>,
    /// Current ordinary cancellation notification.
    cancel: RefCell<Option<Rc<dyn Fn()>>>,
    /// Current label-edit notification.
    label: RefCell<Option<LabelCallback>>,
}
impl TreeList {
    /// Retains the supplied tree, selecting the explicit target or leaf.
    ///
    /// # Errors
    /// Returns invalid data when a reached message or content read is malformed.
    pub fn new(
        tree: &[maestro_app::SessionTreeNode<'_>],
        current_leaf_id: Option<&str>,
        max_visible_lines: usize,
        initial_selected_id: Option<&str>,
        initial_filter_mode: Option<FilterMode>,
    ) -> io::Result<Self> {
        Ok(Self {
            state: RefCell::new(Rc::new(TreeState::new(
                tree,
                current_leaf_id,
                max_visible_lines,
                initial_selected_id,
                initial_filter_mode,
            )?)),
            select: RefCell::new(None),
            cancel: RefCell::new(None),
            label: RefCell::new(None),
        })
    }
    /// Returns a shared selected occurrence, or none for an empty view.
    #[must_use]
    pub fn get_selected_node(&self) -> Option<SessionTreeNode> {
        self.state.borrow().get_selected_node()
    }
    /// Returns the authored query, without token normalization.
    #[must_use]
    pub fn get_search_query(&self) -> String {
        self.state.borrow().query.clone()
    }
    /// Applies live registered keys before accepting printable query input.
    ///
    /// # Errors
    /// Returns invalid data for reached malformed content during filtering.
    pub fn handle_input(&self, key_data: &str) -> io::Result<()> {
        let notification = Rc::make_mut(&mut self.state.borrow_mut()).handle_input(key_data)?;
        match notification {
            Some(Notification::Select(id)) => {
                if let Some(callback) = self.on_select() {
                    callback(&id);
                }
            }
            Some(Notification::Cancel) => {
                if let Some(callback) = self.on_cancel() {
                    callback();
                }
            }
            Some(Notification::Label(id, label)) => {
                if let Some(callback) = self.on_label_edit() {
                    callback(&id, label);
                }
            }
            None => {}
        }
        Ok(())
    }
    /// Renders a stable navigation snapshot with live theme and host observations.
    ///
    /// # Errors
    /// Returns invalid data for reached malformed display fields or theme failures.
    pub fn render(
        &self,
        width: usize,
        theme: &LiveTheme,
        runtime: &dyn TuiRuntime,
        styles_enabled: bool,
    ) -> io::Result<Vec<String>> {
        let state = Rc::clone(&self.state.borrow());
        state.render(width, theme, runtime, styles_enabled)
    }
    /// Installs the panel clock without changing the list constructor.
    pub(super) fn set_clock(&self, now: Rc<dyn Fn() -> std::time::SystemTime>) {
        let old = std::mem::replace(&mut Rc::make_mut(&mut self.state.borrow_mut()).now, now);
        drop(old);
    }
    /// Updates only the first navigation occurrence, without rebuilding the view.
    pub fn update_node_label(&self, entry_id: &str, label: Option<&str>, timestamp: Option<&str>) {
        let (node, now) = {
            let state = self.state.borrow();
            (
                state
                    .nodes
                    .iter()
                    .find(|n| n.entry()["id"] == entry_id)
                    .cloned(),
                state.now.clone(),
            )
        };
        let Some(node) = node else {
            return;
        };
        *node.label_mut() = label.map(|s| Value::String(s.to_owned()));
        let timestamp = label.filter(|s| !s.is_empty()).map(|_| {
            timestamp.map_or_else(
                || {
                    chrono::DateTime::<chrono::Utc>::from(now())
                        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                },
                str::to_owned,
            )
        });
        *node.label_timestamp_mut() = timestamp.map(Value::String);
    }

    /// Leaves this uncached view unchanged.
    pub fn invalidate(&self) {}
    /// Returns the installed confirmation callback.
    #[must_use]
    pub fn on_select(&self) -> Option<SelectCallback> {
        self.select.borrow().clone()
    }
    /// Replaces or removes the confirmation callback.
    pub fn set_on_select(&self, callback: Option<SelectCallback>) {
        let old = self.select.replace(callback);
        drop(old);
    }
    /// Returns the installed cancellation callback.
    #[must_use]
    pub fn on_cancel(&self) -> Option<Rc<dyn Fn()>> {
        self.cancel.borrow().clone()
    }
    /// Replaces or removes the cancellation callback.
    pub fn set_on_cancel(&self, callback: Option<Rc<dyn Fn()>>) {
        let old = self.cancel.replace(callback);
        drop(old);
    }
    /// Returns the installed label-edit callback.
    #[must_use]
    pub fn on_label_edit(&self) -> Option<LabelCallback> {
        self.label.borrow().clone()
    }
    /// Replaces or removes the label-edit callback.
    pub fn set_on_label_edit(&self, callback: Option<LabelCallback>) {
        let old = self.label.replace(callback);
        drop(old);
    }
}
