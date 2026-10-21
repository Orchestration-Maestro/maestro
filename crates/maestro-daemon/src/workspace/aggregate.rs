//! Terminal-aware attention ordering and pane detail projections.

use crate::terminal::state::TerminalState;
use maestro_detection::agents::{Agent, AgentState};
use maestro_workspace::{layout::PaneId, tab::Tab, terminal_id::TerminalId, workspace::Workspace};
use std::collections::HashMap;

/// Agent presentation for one attached pane in layout order.
pub struct PaneDetail {
    /// Durable pane identity.
    pub pane_id: PaneId,
    /// Tab vector position, not its stable public number.
    pub tab_idx: usize,
    /// Current tab display name.
    pub tab_label: String,
    /// Agent label qualified by tab when multiple tabs exist.
    pub label: String,
    /// Effective presentation title, falling back to the manual label.
    pub pane_label: Option<String>,
    /// Latest raw terminal title.
    pub terminal_title: Option<String>,
    /// Latest sanitized terminal title.
    pub terminal_title_stripped: Option<String>,
    /// Effective display-agent override, name or detected label.
    pub agent_label: String,
    /// Effective detected or reported agent kind.
    pub agent_kind_label: Option<String>,
    /// Recognized agent identity, when available.
    pub agent: Option<Agent>,
    /// Effective terminal state.
    pub state: AgentState,
    /// Viewport acknowledgement of the last completion.
    pub seen: bool,
    /// Last published state-transition sequence.
    pub last_agent_state_change_seq: Option<u64>,
    /// Effective state label overrides.
    pub state_labels: HashMap<String, String>,
    /// Current metadata token values.
    pub tokens: HashMap<String, String>,
}

/// Projects an attached agent pane without searching the terminal collection.
fn pane_detail<S: std::hash::BuildHasher>(
    pane_id: PaneId,
    tab: &Tab,
    terminals: &HashMap<TerminalId, TerminalState, S>,
    tab_idx: usize,
    tab_label: &str,
) -> Option<PaneDetail> {
    let pane = tab.panes.get(&pane_id)?;
    let terminal = terminals.get(&pane.attached_terminal_id)?;
    let agent_kind_label = terminal.effective_agent_label().map(str::to_owned);
    let fallback = terminal
        .agent_name
        .as_deref()
        .or(agent_kind_label.as_deref())?;
    let agent_label = terminal
        .effective_display_agent()
        .unwrap_or_else(|| fallback.into());
    let presentation = terminal.effective_presentation();
    Some(PaneDetail {
        pane_id,
        tab_idx,
        tab_label: tab_label.into(),
        label: agent_label.clone(),
        pane_label: terminal
            .effective_title()
            .or_else(|| terminal.manual_label.clone()),
        terminal_title: terminal.terminal_title.clone(),
        terminal_title_stripped: terminal.terminal_title_stripped(),
        agent_label,
        agent_kind_label,
        agent: terminal.effective_known_agent(),
        state: terminal.state,
        seen: pane.seen,
        last_agent_state_change_seq: terminal.last_agent_state_change_seq,
        state_labels: presentation.state_labels,
        tokens: terminal.metadata_tokens.values(),
    })
}

/// Ranks blocked, unseen completion, working, acknowledged completion, then unknown.
fn pane_attention_priority(state: AgentState, seen: bool) -> u8 {
    match (state, seen) {
        (AgentState::Blocked, _) => 4,
        (AgentState::Idle, false) => 3,
        (AgentState::Working, _) => 2,
        (AgentState::Idle, true) => 1,
        (AgentState::Unknown, _) => 0,
    }
}

/// Selects the most urgent attached terminal state and its viewport acknowledgement.
#[must_use]
pub fn aggregate_state<S: std::hash::BuildHasher>(
    workspace: &Workspace,
    terminals: &HashMap<TerminalId, TerminalState, S>,
) -> (AgentState, bool) {
    workspace
        .tabs
        .iter()
        .flat_map(|tab| tab.panes.values())
        .filter_map(|pane| {
            terminals
                .get(&pane.attached_terminal_id)
                .map(|terminal| (terminal.state, pane.seen))
        })
        .max_by_key(|(state, seen)| pane_attention_priority(*state, *seen))
        .unwrap_or((AgentState::Unknown, true))
}

/// Lists agent panes in tab and layout order, qualifying labels in multi-tab workspaces.
#[must_use]
pub fn pane_details<S: std::hash::BuildHasher>(
    workspace: &Workspace,
    terminals: &HashMap<TerminalId, TerminalState, S>,
) -> Vec<PaneDetail> {
    let multi_tab = workspace.tabs.len() > 1;
    workspace
        .tabs
        .iter()
        .enumerate()
        .flat_map(|(tab_idx, tab)| {
            let tab_label = workspace
                .tab_display_name(tab_idx)
                .unwrap_or_else(|| (tab_idx + 1).to_string());
            tab.layout.pane_ids().into_iter().filter_map(move |pane| {
                let mut detail = pane_detail(pane, tab, terminals, tab_idx, &tab_label)?;
                if multi_tab {
                    detail.label = format!("{}·{}", detail.tab_label, detail.agent_label);
                }
                Some(detail)
            })
        })
        .collect()
}
