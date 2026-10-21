//! Workspace tab and pane mutations.
use super::{TakenPane, Workspace};
use crate::{
    layout::{PaneId, SplitRequest},
    pane_state::PaneState,
    tab::{MovedPane, Tab},
    terminal_id::TerminalId,
};

impl Workspace {
    /// Return the stable public number for a registered pane.
    #[must_use]
    pub fn public_pane_number(&self, pane_id: PaneId) -> Option<usize> {
        self.public_pane_numbers.get(&pane_id).copied()
    }
    /// Return a tab’s stable public number by display position.
    #[must_use]
    pub fn public_tab_number(&self, tab_idx: usize) -> Option<usize> {
        self.tabs.get(tab_idx).map(|tab| tab.number)
    }
    /// Find the tab containing a live pane.
    #[must_use]
    pub fn find_tab_index_for_pane(&self, pane_id: PaneId) -> Option<usize> {
        self.tabs
            .iter()
            .position(|tab| tab.panes.contains_key(&pane_id))
    }
    /// Borrow the viewport state of a live pane.
    #[must_use]
    pub fn pane_state(&self, pane_id: PaneId) -> Option<&PaneState> {
        self.tabs.iter().find_map(|tab| tab.panes.get(&pane_id))
    }
    /// Mutably borrow the viewport state of a live pane.
    pub fn pane_state_mut(&mut self, pane_id: PaneId) -> Option<&mut PaneState> {
        self.tabs
            .iter_mut()
            .find_map(|tab| tab.panes.get_mut(&pane_id))
    }
    /// Borrow the terminal attached to a live pane.
    #[must_use]
    pub fn terminal_id(&self, pane_id: PaneId) -> Option<&TerminalId> {
        self.tabs.iter().find_map(|tab| tab.terminal_id(pane_id))
    }
    /// Return the selected pane, or none for an empty transfer source.
    #[must_use]
    pub fn focused_pane_id(&self) -> Option<PaneId> {
        self.active_tab().map(|tab| tab.layout.focused())
    }
    /// Remove a pane, reporting whether the caller should close the workspace.
    pub fn remove_pane(&mut self, pane_id: PaneId) -> bool {
        let Some(tab_idx) = self.find_tab_index_for_pane(pane_id) else {
            return false;
        };
        let pane_count = self.tabs[tab_idx].layout.pane_count();
        let tab_count = self.tabs.len();
        if pane_count <= 1 {
            if tab_count <= 1 {
                return true;
            }
            self.tabs.remove(tab_idx);
            self.unregister_pane(pane_id);
            self.adjust_active_tab_after_removal(tab_idx);
            return false;
        }

        if let Some((removed, _terminal_id)) = self.tabs[tab_idx].remove_pane(pane_id) {
            self.unregister_pane(removed);
        }
        false
    }
    /// Remove a public mapping without recycling its number.
    fn unregister_pane(&mut self, pane_id: PaneId) {
        self.public_pane_numbers.remove(&pane_id);
    }

    /// Close a pane, reporting whether the caller should close the workspace.
    pub fn close_pane(&mut self, pane: PaneId) -> bool {
        self.remove_pane(pane)
    }

    /// Borrow the selected tab, including an empty transfer source.
    #[must_use]
    pub fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active_tab)
    }

    /// Close a tab unless it is the workspace’s last tab.
    pub fn close_tab(&mut self, idx: usize) -> bool {
        if self.tabs.len() <= 1 || idx >= self.tabs.len() {
            return false;
        }
        let tab = self.tabs.remove(idx);
        for pane_id in tab.panes.keys() {
            self.unregister_pane(*pane_id);
        }
        self.adjust_active_tab_after_removal(idx);
        true
    }

    /// Mutably borrow the selected tab.
    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active_tab)
    }
    /// Return the selected tab’s display position.
    #[must_use]
    pub fn active_tab_index(&self) -> usize {
        self.active_tab
    }
    /// Select a valid tab and mark its panes seen.
    pub fn switch_tab(&mut self, idx: usize) {
        let Some(tab) = self.tabs.get_mut(idx) else {
            return;
        };
        self.active_tab = idx;
        for pane in tab.panes.values_mut() {
            pane.seen = true;
        }
    }
    /// Move a tab to an insertion position, preserving the selected tab identity.
    pub fn move_tab(&mut self, source_idx: usize, insert_idx: usize) -> bool {
        if source_idx >= self.tabs.len() || insert_idx > self.tabs.len() {
            return false;
        }

        let target_idx = if source_idx < insert_idx {
            insert_idx.saturating_sub(1)
        } else {
            insert_idx
        }
        .min(self.tabs.len().saturating_sub(1));

        if source_idx == target_idx {
            return false;
        }

        let active_root_pane = self.tabs.get(self.active_tab).map(|tab| tab.root_pane);
        let tab = self.tabs.remove(source_idx);
        self.tabs.insert(target_idx, tab);
        self.active_tab = active_root_pane
            .and_then(|root_pane| self.tabs.iter().position(|tab| tab.root_pane == root_pane))
            .unwrap_or(target_idx);
        true
    }

    /// Repair the selection after removing a tab.
    fn adjust_active_tab_after_removal(&mut self, removed_idx: usize) {
        if self.tabs.is_empty() {
            self.active_tab = 0;
        } else if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        } else if removed_idx <= self.active_tab && self.active_tab > 0 {
            self.active_tab -= 1;
        }
    }
    /// Take a pane while retaining its public mapping until explicit unregister.
    pub fn take_pane_for_move(&mut self, pane_id: PaneId) -> Option<TakenPane> {
        let tab_idx = self.find_tab_index_for_pane(pane_id)?;
        let pane_count = self.tabs[tab_idx].layout.pane_count();
        if pane_count <= 1 {
            let mut tab = self.tabs.remove(tab_idx);
            let moved = tab.take_pane_for_move(pane_id)?;
            self.adjust_active_tab_after_removal(tab_idx);
            return Some(TakenPane {
                moved,
                removed_tab_idx: Some(tab_idx),
                workspace_empty: self.tabs.is_empty(),
            });
        }

        let moved = self.tabs[tab_idx].take_pane_for_move(pane_id)?;
        Some(TakenPane {
            moved,
            removed_tab_idx: None,
            workspace_empty: false,
        })
    }
    /// Insert a transferred pane and register its public number only when absent.
    ///
    /// # Errors
    /// Returns the original record when the tab, target or identity is rejected.
    pub fn insert_moved_pane_into_tab(
        &mut self,
        tab: usize,
        moved: MovedPane,
        request: &SplitRequest,
    ) -> Result<PaneId, MovedPane> {
        if self
            .tabs
            .iter()
            .any(|tab| tab.panes.contains_key(&moved.pane_id))
        {
            return Err(moved);
        }
        let Some(tab) = self.tabs.get_mut(tab) else {
            return Err(moved);
        };
        let pane = tab.insert_existing_pane(moved, request)?;
        if !self.public_pane_numbers.contains_key(&pane) {
            self.register_new_pane_with_number(pane, self.next_public_pane_number);
        }
        Ok(pane)
    }
    /// Keep the transfer source mapping until publication in the destination succeeds.
    pub fn unregister_moved_pane(&mut self, pane: PaneId) {
        self.unregister_pane(pane);
    }
    /// Recover a transferred pane as a new tab, including in an empty source workspace.
    pub fn create_tab_from_existing_pane(
        &mut self,
        moved: MovedPane,
        label: Option<String>,
    ) -> usize {
        let number = self.next_public_tab_number;
        self.next_public_tab_number += 1;
        if !self.public_pane_numbers.contains_key(&moved.pane_id) {
            self.register_new_pane_with_number(moved.pane_id, self.next_public_pane_number);
        }
        self.tabs
            .push(Tab::from_existing_pane(number, label, moved));
        self.tabs.len() - 1
    }

    /// Set an explicit name, including an empty name.
    pub fn set_custom_name(&mut self, name: String) {
        self.custom_name = Some(name);
    }
    /// Display the explicit name or the admitted identity label.
    #[must_use]
    pub fn display_name(&self, home: Option<&std::path::Path>) -> String {
        self.display_name_for_cwd(&self.identity_cwd, home)
    }
    /// Return an explicit name or the automatic label for a supplied live directory.
    #[must_use]
    pub fn display_name_for_cwd(
        &self,
        cwd: &std::path::Path,
        home: Option<&std::path::Path>,
    ) -> String {
        self.custom_name
            .clone()
            .unwrap_or_else(|| self.automatic_display_name_for_cwd(cwd, home))
    }
    /// Use the admitted label only for its supplied identity path.
    fn automatic_display_name_for_cwd(
        &self,
        cwd: &std::path::Path,
        home: Option<&std::path::Path>,
    ) -> String {
        if cwd == self.cached_identity_cwd {
            self.cached_auto_label.clone()
        } else {
            super::git::fallback_label_from_cwd(cwd, home)
        }
    }

    /// Return an explicit name or its one-based display position.
    #[must_use]
    pub fn tab_display_name(&self, tab_idx: usize) -> Option<String> {
        let tab = self.tabs.get(tab_idx)?;
        Some(
            tab.custom_name
                .clone()
                .unwrap_or_else(|| (tab_idx + 1).to_string()),
        )
    }
    /// Return the selected tab’s display name.
    #[must_use]
    pub fn active_tab_display_name(&self) -> Option<String> {
        self.tab_display_name(self.active_tab)
    }

    /// Return the admitted branch without discovery.
    #[must_use]
    pub fn branch(&self) -> Option<String> {
        self.cached_git_branch.clone()
    }
    /// Return admitted upstream counts.
    #[must_use]
    pub fn git_ahead_behind(&self) -> Option<(usize, usize)> {
        self.cached_git_ahead_behind
    }
    /// Borrow admitted repository grouping.
    #[must_use]
    pub fn git_space(&self) -> Option<&super::GitSpaceMetadata> {
        self.cached_git_space.as_ref()
    }
    /// Borrow explicit worktree provenance.
    #[must_use]
    pub fn worktree_space(&self) -> Option<&super::WorktreeSpaceMembership> {
        self.worktree_space.as_ref()
    }
}
