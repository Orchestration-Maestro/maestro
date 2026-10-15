//! Configurable input actions on the retained navigation view.
use super::{FilterMode, tree_list::TreeState};
use maestro_tui::{KeybindingsManager, get_keybindings};
use serde_json::Value;
use std::{collections::HashSet, io};

/// Owned notification arguments dispatched after releasing the state borrow.
pub(super) enum Notification {
    /// Confirmed selected identity.
    Select(String),
    /// Ordinary cancellation.
    Cancel,
    /// Selected identity and raw optional label.
    Label(String, Option<Value>),
}
impl TreeState {
    /// Applies an input using the active toolkit keybinding manager.
    ///
    /// # Errors
    /// Returns invalid data when filtering reaches malformed message content.
    pub(super) fn handle_input(&mut self, key_data: &str) -> io::Result<Option<Notification>> {
        let keys = get_keybindings();
        let last = self.visible.len().saturating_sub(1);
        if keys.matches(key_data, "tui.select.up") {
            self.selected = self.selected.checked_sub(1).unwrap_or(last);
        } else if keys.matches(key_data, "tui.select.down") {
            self.selected = if self.selected >= last {
                0
            } else {
                self.selected + 1
            };
        } else if keys.matches(key_data, "app.tree.foldOrUp") {
            self.fold_or_jump(Direction::Up)?;
        } else if keys.matches(key_data, "app.tree.unfoldOrDown") {
            self.fold_or_jump(Direction::Down)?;
        } else if keys.matches(key_data, "tui.editor.cursorLeft")
            || keys.matches(key_data, "tui.select.pageUp")
        {
            self.selected = self.selected.saturating_sub(self.max_visible_lines);
        } else if keys.matches(key_data, "tui.editor.cursorRight")
            || keys.matches(key_data, "tui.select.pageDown")
        {
            self.selected = self
                .selected
                .saturating_add(self.max_visible_lines)
                .min(last);
        } else if keys.matches(key_data, "tui.select.confirm") {
            return Ok(self.get_selected_node().and_then(|node| {
                node.entry()["id"]
                    .as_str()
                    .map(|id| Notification::Select(id.to_owned()))
            }));
        } else if keys.matches(key_data, "tui.select.cancel") {
            if self.query.is_empty() {
                return Ok(Some(Notification::Cancel));
            }
            self.query.clear();
            self.reset_folds()?;
        } else if !self.handle_filter_input(&keys, key_data)? {
            return self.handle_query_input(&keys, key_data);
        }
        self.remember_selection();
        Ok(None)
    }

    /// Handles editing and notifications only after registered navigation/filter actions.
    fn handle_query_input(
        &mut self,
        keys: &KeybindingsManager,
        data: &str,
    ) -> io::Result<Option<Notification>> {
        if keys.matches(data, "tui.editor.deleteCharBackward") {
            if self.query.pop().is_some() {
                self.reset_folds()?;
            }
        } else if keys.matches(data, "app.tree.editLabel") {
            if let Some(node) = self.get_selected_node()
                && let Some(id) = node.entry()["id"].as_str()
            {
                return Ok(Some(Notification::Label(
                    id.to_owned(),
                    node.label().clone(),
                )));
            }
        } else if keys.matches(data, "app.tree.toggleLabelTimestamp") {
            self.show_label_timestamps = !self.show_label_timestamps;
        } else if !data.is_empty() && !data.chars().any(char::is_control) {
            self.query.push_str(data);
            self.reset_folds()?;
        }
        Ok(None)
    }
    /// Query and category changes reopen all folded segments.
    fn reset_folds(&mut self) -> io::Result<()> {
        self.folded.clear();
        self.apply_filter()
    }

    /// Folds an eligible segment or walks the visible structure.
    fn fold_or_jump(&mut self, direction: Direction) -> io::Result<()> {
        let Some(node) = self.get_selected_node() else {
            return Ok(());
        };
        let Some(id) = node.entry()["id"].as_str() else {
            return Ok(());
        };
        let changed = match direction {
            Direction::Up => self.structure.is_foldable(id) && self.folded.insert(id.to_owned()),
            Direction::Down => self.folded.remove(id),
        };
        if changed {
            self.apply_filter()?;
        } else {
            self.selected = self.segment(id, direction);
        }
        Ok(())
    }
    /// Walks semantic identities, preserving selection when no boundary is reachable.
    fn segment(&self, id: &str, direction: Direction) -> usize {
        let indices: std::collections::HashMap<_, _> = self
            .visible
            .iter()
            .enumerate()
            .filter_map(|(i, node)| node.entry()["id"].as_str().map(|id| (id, i)))
            .collect();
        let mut current = id;
        let mut visited = HashSet::new();
        while visited.insert(current) {
            match self.segment_transition(current, direction, &indices) {
                std::ops::ControlFlow::Continue(next) => current = next,
                std::ops::ControlFlow::Break(index) => return index,
            }
        }
        self.selected
    }
    /// Advances one semantic edge or reports the reached branch boundary.
    fn segment_transition<'a>(
        &'a self,
        current: &'a str,
        direction: Direction,
        indices: &std::collections::HashMap<&str, usize>,
    ) -> std::ops::ControlFlow<usize, &'a str> {
        use std::ops::ControlFlow::{Break, Continue};
        match direction {
            Direction::Down => {
                let Some(children) = self
                    .structure
                    .children
                    .get(&Some(current.to_owned()))
                    .filter(|c| !c.is_empty())
                else {
                    return Break(indices[current]);
                };
                if children.len() > 1 {
                    return Break(indices[children[0].as_str()]);
                }
                Continue(&children[0])
            }
            Direction::Up => {
                let Some(parent) = self
                    .structure
                    .parents
                    .get(current)
                    .and_then(Option::as_deref)
                else {
                    return Break(indices[current]);
                };
                if self
                    .structure
                    .children
                    .get(&Some(parent.to_owned()))
                    .is_some_and(|c| c.len() > 1)
                    && indices[current] < self.selected
                {
                    return Break(indices[current]);
                }
                Continue(parent)
            }
        }
    }

    /// Resolves direct filter actions before backward and forward cycling.
    fn handle_filter_input(
        &mut self,
        keys: &KeybindingsManager,
        key_data: &str,
    ) -> io::Result<bool> {
        let direct = [
            ("app.tree.filter.default", FilterMode::Default),
            ("app.tree.filter.noTools", FilterMode::NoTools),
            ("app.tree.filter.userOnly", FilterMode::UserOnly),
            ("app.tree.filter.labeledOnly", FilterMode::LabeledOnly),
            ("app.tree.filter.all", FilterMode::All),
        ]
        .into_iter()
        .find(|(action, _)| keys.matches(key_data, action));
        if let Some((_, mode)) = direct {
            self.mode = if self.mode == mode {
                FilterMode::Default
            } else {
                mode
            };
        } else if keys.matches(key_data, "app.tree.filter.cycleBackward") {
            self.mode = cycle(self.mode, true);
        } else if keys.matches(key_data, "app.tree.filter.cycleForward") {
            self.mode = cycle(self.mode, false);
        } else {
            return Ok(false);
        }
        self.reset_folds()?;
        Ok(true)
    }
}

/// Moves one position in the five-mode cycle without a second action registry.
fn cycle(mode: FilterMode, backward: bool) -> FilterMode {
    use FilterMode::{All, Default, LabeledOnly, NoTools, UserOnly};
    match (mode, backward) {
        (Default, false) | (UserOnly, true) => NoTools,
        (NoTools, false) | (LabeledOnly, true) => UserOnly,
        (UserOnly, false) | (All, true) => LabeledOnly,
        (LabeledOnly, false) | (Default, true) => All,
        (All, false) | (NoTools, true) => Default,
    }
}

/// Direction along the visible branch segment.
#[derive(Clone, Copy)]
enum Direction {
    /// Walk visible parents or fold.
    Up,
    /// Walk first children or unfold.
    Down,
}
