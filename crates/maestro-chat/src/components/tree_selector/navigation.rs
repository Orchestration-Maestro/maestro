//! Configurable input actions on the borrowed navigation view.
use super::{FilterMode, TreeList};
use maestro_tui::{KeybindingsManager, get_keybindings};
use std::io;

impl TreeList<'_, '_> {
    /// Applies an input using the active toolkit keybinding manager.
    ///
    /// # Errors
    /// Returns invalid data when filtering reaches malformed message content.
    pub fn handle_input(&mut self, key_data: &str) -> io::Result<()> {
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
            if let Some(node) = self.get_selected_node()
                && let Some(select) = &mut self.on_select
                && let Some(id) = node.entry["id"].as_str()
            {
                select(id);
            }
        } else if keys.matches(key_data, "tui.select.cancel") {
            if let Some(cancel) = &mut self.on_cancel {
                cancel();
            }
        } else {
            self.handle_filter_input(&keys, key_data)?;
        }
        self.remember_selection();
        Ok(())
    }

    /// Resolves direct filter actions before backward and forward cycling.
    fn handle_filter_input(&mut self, keys: &KeybindingsManager, key_data: &str) -> io::Result<()> {
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
            return Ok(());
        }
        self.apply_filter()
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
