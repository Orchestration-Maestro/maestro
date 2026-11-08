//! Terminal and agent targeting keep stable pane handles distinct from names.

use super::{App, actions::pane_agent_status};
use crate::terminal::model::TerminalState;
use maestro_protocol::schema::AgentStatus;
use maestro_workspace::tiling::PaneId;

/// Located viewport and its attached terminal identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTarget {
    /// Workspace display position.
    pub ws_idx: usize,
    /// Tab display position.
    pub tab_idx: usize,
    /// Durable pane identity.
    pub pane_id: PaneId,
    /// Durable terminal handle.
    pub terminal_id: String,
}

/// Public observations identifying one ambiguous target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTargetCandidate {
    /// Durable terminal handle.
    pub terminal_id: String,
    /// Current stable public pane handle.
    pub pane_id: String,
    /// Stable workspace handle.
    pub workspace_id: String,
    /// Stable public tab handle.
    pub tab_id: String,
    /// Live process directory, falling back to retained metadata.
    pub cwd: Option<String>,
    /// Effective execution and acknowledgement status.
    pub agent_status: AgentStatus,
}

/// Target lookup failed without choosing an arbitrary matching pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalTargetError {
    /// No current terminal or agent matches the supplied form.
    NotFound {
        /// Original supplied form.
        target: String,
    },
    /// Multiple viewports match the supplied terminal or name.
    Ambiguous {
        /// Original supplied form.
        target: String,
        /// Public observations of the matching viewports.
        candidates: Vec<TerminalTargetCandidate>,
    },
}

impl<C: FnMut() -> u128> App<C> {
    /// Resolves terminal identity, current public pane identity, then agent name or label.
    ///
    /// # Errors
    /// Returns the missing target or all matching candidates when ambiguous.
    pub fn resolve_terminal_target(
        &self,
        target: &str,
    ) -> Result<TerminalTarget, TerminalTargetError> {
        let matches = self
            .terminal_targets()
            .into_iter()
            .filter(|candidate| candidate.terminal_id == target)
            .collect();
        if let Some(resolved) = self.single_terminal_match(target, matches)? {
            return Ok(resolved);
        }
        if let Some((workspace, pane)) = self.state.parse_current_public_pane_id(target)
            && let Some(resolved) = self.terminal_target_for_pane(workspace, pane)
        {
            return Ok(resolved);
        }
        let matches = self
            .terminal_targets()
            .into_iter()
            .filter(|candidate| {
                self.target_terminal(candidate).is_some_and(|terminal| {
                    terminal.agent_name.as_deref() == Some(target)
                        || terminal.effective_agent_label() == Some(target)
                })
            })
            .collect();
        self.single_terminal_match(target, matches)?
            .ok_or_else(|| TerminalTargetError::NotFound {
                target: target.into(),
            })
    }

    /// Resolves an agent pane's current public identity or an explicit agent name.
    ///
    /// # Errors
    /// Returns the missing target or all matching names when ambiguous.
    pub fn resolve_agent_target(
        &self,
        target: &str,
    ) -> Result<TerminalTarget, TerminalTargetError> {
        if let Some((workspace, pane)) = self.state.parse_current_public_pane_id(target)
            && let Some(resolved) = self.terminal_target_for_pane(workspace, pane)
            && self
                .target_terminal(&resolved)
                .is_some_and(TerminalState::is_agent_terminal)
        {
            return Ok(resolved);
        }
        let matches = self
            .terminal_targets()
            .into_iter()
            .filter(|candidate| {
                self.target_terminal(candidate)
                    .is_some_and(|terminal| terminal.agent_name.as_deref() == Some(target))
            })
            .collect();
        self.single_terminal_match(target, matches)?
            .ok_or_else(|| TerminalTargetError::NotFound {
                target: target.into(),
            })
    }

    /// Borrows metadata through the attached identity rather than scanning every terminal.
    fn target_terminal(&self, target: &TerminalTarget) -> Option<&TerminalState> {
        let terminal = self
            .state
            .topology
            .workspaces
            .get(target.ws_idx)?
            .terminal_id(target.pane_id)?;
        self.state.terminals.get(terminal)
    }

    /// Converts multiple matches to public observations without choosing one by ordering.
    fn single_terminal_match(
        &self,
        target: &str,
        matches: Vec<TerminalTarget>,
    ) -> Result<Option<TerminalTarget>, TerminalTargetError> {
        match matches.len() {
            0 => Ok(None),
            1 => Ok(matches.into_iter().next()),
            _ => Err(TerminalTargetError::Ambiguous {
                target: target.into(),
                candidates: matches
                    .into_iter()
                    .filter_map(|candidate| {
                        self.terminal_target_candidate(candidate.ws_idx, candidate.pane_id)
                    })
                    .collect(),
            }),
        }
    }

    /// Enumerates current layout viewports, retaining duplicate terminal attachments.
    fn terminal_targets(&self) -> Vec<TerminalTarget> {
        self.state
            .topology
            .workspaces
            .iter()
            .enumerate()
            .flat_map(|(ws_idx, workspace)| {
                workspace
                    .tabs
                    .iter()
                    .enumerate()
                    .map(move |(tab_idx, tab)| (ws_idx, tab_idx, tab))
            })
            .flat_map(|(ws_idx, tab_idx, tab)| {
                tab.layout
                    .pane_ids()
                    .into_iter()
                    .filter_map(move |pane_id| {
                        tab.terminal_id(pane_id).map(|terminal| TerminalTarget {
                            ws_idx,
                            tab_idx,
                            pane_id,
                            terminal_id: terminal.to_string(),
                        })
                    })
            })
            .collect()
    }

    /// Locates the attached terminal for a pane in its current tab.
    fn terminal_target_for_pane(&self, ws_idx: usize, pane_id: PaneId) -> Option<TerminalTarget> {
        let workspace = self.state.topology.workspaces.get(ws_idx)?;
        Some(TerminalTarget {
            ws_idx,
            tab_idx: workspace.find_tab_index_for_pane(pane_id)?,
            pane_id,
            terminal_id: workspace.terminal_id(pane_id)?.to_string(),
        })
    }

    /// Captures one matching viewport's public identity, directory and agent status.
    fn terminal_target_candidate(
        &self,
        ws_idx: usize,
        pane_id: PaneId,
    ) -> Option<TerminalTargetCandidate> {
        let workspace = self.state.topology.workspaces.get(ws_idx)?;
        let tab_idx = workspace.find_tab_index_for_pane(pane_id)?;
        let pane = workspace.pane_state(pane_id)?;
        let terminal = self.state.terminals.get(&pane.attached_terminal_id)?;
        let cwd = crate::tab::cwd_for_pane(
            &workspace.tabs[tab_idx],
            pane_id,
            &self.state.terminals,
            &self.terminal_runtimes,
        );
        Some(TerminalTargetCandidate {
            terminal_id: terminal.id.to_string(),
            pane_id: self.state.public_pane_id(ws_idx, pane_id)?,
            workspace_id: workspace.id.clone(),
            tab_id: self.state.public_tab_id(ws_idx, tab_idx)?,
            cwd: cwd.map(|cwd| cwd.display().to_string()),
            agent_status: pane_agent_status(terminal.state, pane.seen),
        })
    }
}
