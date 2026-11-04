//! Public identity parsing over the topology owner's stable numbers.

use super::state::DaemonState;
use maestro_workspace::{
    layout::PaneId,
    pane_state::PaneState,
    workspace::{decode_public_number, public_pane_id_for_number, public_tab_id_for_number},
};

impl<C: FnMut() -> u128> DaemonState<C> {
    /// Resolves a stable workspace handle or a historical positional spelling.
    #[must_use]
    pub fn parse_workspace_id(&self, id: &str) -> Option<usize> {
        self.topology
            .workspaces
            .iter()
            .position(|workspace| {
                #[cfg(test)]
                crate::tests::record_workspace_work();
                workspace.id == id
            })
            .or_else(|| parse_positional_workspace_id(id))
    }

    /// Resolves stable `w1:t2` numbers or historical positional tab spellings.
    #[must_use]
    pub fn parse_tab_id(&self, id: &str) -> Option<(usize, usize)> {
        let (workspace, tab, numbered) = if let Some(rest) = id.strip_prefix("t_") {
            let (workspace, tab) = rest.rsplit_once('_')?;
            (workspace, tab, false)
        } else {
            let (workspace, tab) = id.rsplit_once(':')?;
            (
                workspace,
                tab.strip_prefix('t').unwrap_or(tab),
                tab.starts_with('t'),
            )
        };
        let workspace = self.parse_workspace_id(workspace)?;
        let tabs = &self.topology.workspaces.get(workspace)?.tabs;
        let tab = if numbered {
            let number = decode_public_number(tab)?;
            tabs.iter().position(|tab| tab.number == number)?
        } else {
            tab.parse::<usize>().ok()?.checked_sub(1)?
        };
        tabs.get(tab)?;
        Some((workspace, tab))
    }
}

impl<C: FnMut() -> u128> DaemonState<C> {
    /// Finds the owning workspace and viewport of a live pane.
    #[must_use]
    pub fn find_pane(&self, pane: PaneId) -> Option<(usize, &PaneState)> {
        self.topology
            .workspaces
            .iter()
            .enumerate()
            .find_map(|(index, workspace)| workspace.pane_state(pane).map(|state| (index, state)))
    }

    /// Returns the stable public tab handle at a display position.
    #[must_use]
    pub fn public_tab_id(&self, workspace: usize, tab: usize) -> Option<String> {
        let workspace = self.topology.workspaces.get(workspace)?;
        Some(public_tab_id_for_number(
            &workspace.id,
            workspace.public_tab_number(tab)?,
        ))
    }

    /// Returns the stable public pane handle in its owning workspace.
    #[must_use]
    pub fn public_pane_id(&self, workspace: usize, pane: PaneId) -> Option<String> {
        let workspace = self.topology.workspaces.get(workspace)?;
        Some(public_pane_id_for_number(
            &workspace.id,
            workspace.public_pane_number(pane)?,
        ))
    }

    /// Resolves a historical raw identity without falling through a stale alias.
    fn resolve_raw_pane_id(&self, raw: u32) -> Option<PaneId> {
        let pane = self
            .pane_id_aliases
            .get(&raw)
            .copied()
            .unwrap_or_else(|| PaneId::from_raw(raw));
        self.find_pane(pane).map(|_| pane)
    }

    /// Forgets the historical numeric spelling that a newly admitted pane identity reuses.
    pub fn remove_alias_shadowed_by_new_pane(&mut self, pane: PaneId) {
        self.pane_id_aliases.remove(&pane.raw());
    }

    /// Resolves stable pane numbers and historical raw or public aliases.
    #[must_use]
    pub fn parse_pane_id(&self, id: &str) -> Option<(usize, PaneId)> {
        if let Some(pane) = self.public_pane_id_aliases.get(id).copied() {
            return self.find_pane(pane).map(|(workspace, _)| (workspace, pane));
        }
        if let Some(rest) = id.strip_prefix("p_") {
            if let Some((workspace, raw)) = rest.rsplit_once('_') {
                let workspace = self.parse_workspace_id(workspace)?;
                let pane = self.resolve_raw_pane_id(raw.parse().ok()?)?;
                self.topology.workspaces.get(workspace)?.pane_state(pane)?;
                return Some((workspace, pane));
            }
            let pane = self.resolve_raw_pane_id(rest.parse().ok()?)?;
            return self.find_pane(pane).map(|(workspace, _)| (workspace, pane));
        }
        let (workspace, number) = if let Some((workspace, number)) = id.rsplit_once(":p") {
            (workspace, decode_public_number(number)?)
        } else {
            let (workspace, number) = id.rsplit_once('-')?;
            (workspace, number.parse().ok()?)
        };
        let index = self.parse_workspace_id(workspace)?;
        let workspace = self.topology.workspaces.get(index)?;
        workspace
            .public_pane_numbers
            .iter()
            .find_map(|(pane, candidate)| (*candidate == number).then_some((index, *pane)))
    }

    /// Resolves only the current stable spelling, excluding historical aliases.
    #[must_use]
    pub fn parse_current_public_pane_id(&self, id: &str) -> Option<(usize, PaneId)> {
        let (workspace, pane) = self.parse_pane_id(id)?;
        (self.public_pane_id(workspace, pane).as_deref() == Some(id)).then_some((workspace, pane))
    }
}

/// Resolves a historical `w_N` or bare `N` workspace spelling to its display position,
/// without checking that position.
pub(crate) fn parse_positional_workspace_id(id: &str) -> Option<usize> {
    id.strip_prefix("w_")
        .unwrap_or(id)
        .parse::<usize>()
        .ok()?
        .checked_sub(1)
}
