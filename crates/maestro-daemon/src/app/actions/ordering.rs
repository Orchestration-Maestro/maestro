//! Workspace ordering with identity-based navigation retention.

use super::super::state::DaemonState;

impl<C: FnMut() -> u128> DaemonState<C> {
    /// Moves one workspace before an insertion position, preserving active identities.
    pub fn move_workspace(&mut self, source: usize, insert: usize) -> bool {
        let len = self.topology.workspaces.len();
        if source >= len || insert > len {
            return false;
        }
        let target = if source < insert { insert - 1 } else { insert };
        if source == target {
            return false;
        }
        let active = self
            .active
            .map(|index| self.topology.workspaces[index].id.clone());
        let selected = self
            .topology
            .workspaces
            .get(self.selected)
            .map(|workspace| workspace.id.clone());
        let workspace = self.topology.workspaces.remove(source);
        self.topology.workspaces.insert(target, workspace);
        self.restore_workspace_selection(active.as_deref(), selected.as_deref());
        self.session_dirty = true;
        true
    }

    /// Rebinds positional selection after moving the original logical workspaces.
    fn restore_workspace_selection(&mut self, active: Option<&str>, selected: Option<&str>) {
        self.active = active.and_then(|id| {
            self.topology
                .workspaces
                .iter()
                .position(|workspace| workspace.id == id)
        });
        self.selected = selected
            .and_then(|id| {
                self.topology
                    .workspaces
                    .iter()
                    .position(|workspace| workspace.id == id)
            })
            .unwrap_or(0);
    }
}

impl<C: FnMut() -> u128> DaemonState<C> {
    /// Moves a supplied ordered block before another workspace, or to the end.
    pub fn move_workspace_block(&mut self, ids: &[String], before: Option<&str>) -> bool {
        use std::collections::{HashMap, HashSet};
        let moved: HashSet<_> = ids.iter().map(String::as_str).collect();
        if !self.valid_workspace_block(ids, before, &moved) {
            return false;
        }
        let mut desired: Vec<_> = self
            .topology
            .workspaces
            .iter()
            .filter(|workspace| !moved.contains(workspace.id.as_str()))
            .map(|workspace| workspace.id.as_str())
            .collect();
        let insert = before
            .and_then(|id| desired.iter().position(|candidate| *candidate == id))
            .unwrap_or(desired.len());
        desired.splice(insert..insert, ids.iter().map(String::as_str));
        if self
            .topology
            .workspaces
            .iter()
            .map(|workspace| workspace.id.as_str())
            .eq(desired.iter().copied())
        {
            return false;
        }
        let active = self.active.and_then(|index| {
            desired
                .iter()
                .position(|id| *id == self.topology.workspaces[index].id)
        });
        let selected = self
            .topology
            .workspaces
            .get(self.selected)
            .and_then(|workspace| desired.iter().position(|id| *id == workspace.id))
            .unwrap_or(0);
        let positions: HashMap<_, _> = desired
            .into_iter()
            .enumerate()
            .map(|(position, id)| (id.to_owned(), position))
            .collect();
        self.topology
            .workspaces
            .sort_by_key(|workspace| positions[&workspace.id]);
        self.active = active;
        self.selected = selected;
        self.session_dirty = true;
        true
    }

    /// Validates membership, uniqueness and insertion outside the moved block.
    fn valid_workspace_block(
        &self,
        ids: &[String],
        before: Option<&str>,
        moved: &std::collections::HashSet<&str>,
    ) -> bool {
        !moved.is_empty()
            && moved.len() == ids.len()
            && ids.iter().all(|id| {
                self.topology.workspaces.iter().any(|workspace| {
                    #[cfg(test)]
                    crate::tests::record_workspace_work();
                    workspace.id == *id
                })
            })
            && before.is_none_or(|id| {
                !moved.contains(id)
                    && self
                        .topology
                        .workspaces
                        .iter()
                        .any(|workspace| workspace.id == id)
            })
    }
}

#[cfg(test)]
mod tests;
