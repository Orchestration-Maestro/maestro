//! Owned topology and separate terminal-history snapshots.

use crate::{
    app::state::DaemonState,
    terminal::{runtime_registry::TerminalRuntimeRegistry, state::TerminalState},
};
use maestro_workspace::{
    pane_state::PaneState,
    persist::snapshot::{
        PaneAgentSessionSnapshot, PaneHistorySnapshot, PaneSnapshot, SNAPSHOT_VERSION,
        SessionHistorySnapshot, SessionSnapshot, TabHistorySnapshot, TabSnapshot,
        WorkspaceHistorySnapshot, WorkspaceSnapshot, capture_node,
    },
    tab::Tab,
    terminal_id::TerminalId,
    workspace::Workspace,
};
use std::{collections::HashMap, path::PathBuf};

/// Captures owned topology and pane launch metadata without retaining daemon borrows.
#[must_use]
pub fn capture<C: FnMut() -> u128>(
    state: &DaemonState<C>,
    runtimes: &TerminalRuntimeRegistry,
) -> SessionSnapshot {
    SessionSnapshot {
        version: SNAPSHOT_VERSION,
        workspaces: state
            .topology
            .workspaces
            .iter()
            .map(|workspace| capture_workspace(workspace, &state.terminals, runtimes))
            .collect(),
        active: state.active,
        selected: state.selected,
        sidebar_width: Some(state.sidebar_width),
        sidebar_section_split: Some(state.sidebar_section_split),
        collapsed_space_keys: state.collapsed_space_keys.clone(),
    }
}

/// Captures stable workspace numbering and identity from its first root process.
fn capture_workspace(
    workspace: &Workspace,
    terminals: &HashMap<TerminalId, TerminalState>,
    runtimes: &TerminalRuntimeRegistry,
) -> WorkspaceSnapshot {
    WorkspaceSnapshot {
        id: Some(workspace.id.clone()),
        custom_name: workspace.custom_name.clone(),
        identity_cwd: crate::workspace::resolved_identity_cwd_from(workspace, terminals, runtimes)
            .unwrap_or_else(|| workspace.identity_cwd.clone()),
        worktree_space: workspace.worktree_space.clone(),
        public_pane_numbers: workspace
            .public_pane_numbers
            .iter()
            .map(|(pane, number)| (pane.raw(), *number))
            .collect(),
        next_public_pane_number: workspace.next_public_pane_number,
        public_tab_numbers: workspace.tabs.iter().map(|tab| tab.number).collect(),
        next_public_tab_number: workspace.next_public_tab_number,
        tabs: workspace
            .tabs
            .iter()
            .map(|tab| capture_tab(tab, terminals, runtimes))
            .collect(),
        active_tab: workspace.active_tab,
    }
}

/// Captures the layout and pane records independently of optional screen history.
fn capture_tab(
    tab: &Tab,
    terminals: &HashMap<TerminalId, TerminalState>,
    runtimes: &TerminalRuntimeRegistry,
) -> TabSnapshot {
    let panes = tab
        .panes
        .keys()
        .map(|pane| {
            let cwd = crate::tab::cwd_for_pane(tab, *pane, terminals, runtimes)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")));
            let terminal = tab
                .panes
                .get(pane)
                .and_then(|pane| terminals.get(&pane.attached_terminal_id));
            (pane.raw(), capture_pane(terminal, cwd))
        })
        .collect();
    TabSnapshot {
        custom_name: tab.custom_name.clone(),
        layout: capture_node(tab.layout.root()),
        panes,
        zoomed: tab.zoomed,
        focused: Some(tab.layout.focused().raw()),
        root_pane: Some(tab.root_pane.raw()),
    }
}

/// Captures settled managed ownership and the terminal owner's preferred session provenance.
fn capture_pane(terminal: Option<&TerminalState>, cwd: PathBuf) -> PaneSnapshot {
    let (agent_name, managed_agent_kind) = terminal
        .filter(|terminal| !terminal.managed_agent_launch_pending())
        .map(|terminal| {
            (
                terminal.agent_name.clone(),
                terminal
                    .managed_agent_kind()
                    .map(|agent| maestro_detection::agents::agent_label(agent).to_owned()),
            )
        })
        .unwrap_or_default();
    let agent_session = terminal
        .and_then(TerminalState::current_session_identity_for_persistence)
        .map(|(source, agent, session)| PaneAgentSessionSnapshot {
            source: source.into(),
            agent: agent.into(),
            kind: session.kind,
            value: session.value.clone(),
        });
    PaneSnapshot {
        cwd,
        label: terminal.and_then(|terminal| terminal.manual_label.clone()),
        agent_name,
        managed_agent_kind,
        agent_session,
        launch_argv: terminal.and_then(|terminal| terminal.launch_argv.clone()),
    }
}

/// Captures each pane's primary-screen history separately from session structure.
#[must_use]
pub fn capture_history(
    workspaces: &[Workspace],
    runtimes: &TerminalRuntimeRegistry,
) -> SessionHistorySnapshot {
    SessionHistorySnapshot {
        version: SNAPSHOT_VERSION,
        workspaces: workspaces
            .iter()
            .map(|workspace| WorkspaceHistorySnapshot {
                tabs: workspace
                    .tabs
                    .iter()
                    .map(|tab| TabHistorySnapshot {
                        panes: capture_tab_history(tab, runtimes),
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// Omits panes without a retained runtime history.
fn capture_tab_history(
    tab: &Tab,
    runtimes: &TerminalRuntimeRegistry,
) -> HashMap<u32, PaneHistorySnapshot> {
    tab.panes
        .iter()
        .filter_map(|(pane, state)| {
            capture_pane_history(Some(state), runtimes).map(|history| (pane.raw(), history))
        })
        .collect()
}

/// Takes an owned ANSI history from the runtime attached to a viewport.
fn capture_pane_history(
    pane: Option<&PaneState>,
    runtimes: &TerminalRuntimeRegistry,
) -> Option<PaneHistorySnapshot> {
    let ansi = runtimes
        .get(&pane?.attached_terminal_id)?
        .snapshot_history()?;
    let lines = ansi.lines().count();
    Some(PaneHistorySnapshot { ansi, lines })
}
