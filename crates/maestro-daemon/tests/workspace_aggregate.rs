//! Terminal-aware workspace attention and ordered pane detail projections.
#![cfg(test)]
#![cfg(target_os = "linux")]

use maestro_daemon::{terminal::state::TerminalState, workspace::aggregate};
use maestro_detection::agents::{Agent, AgentState};
use maestro_workspace::{
    layout::PaneId,
    workspace::{Workspace, WorkspaceTopology},
};
use std::collections::HashMap;

fn topology() -> WorkspaceTopology<fn() -> u128> {
    let mut topology = WorkspaceTopology::new((|| 0) as fn() -> u128);
    topology
        .new_with_tab("/tmp".into(), None, |_, _| Ok(()))
        .unwrap();
    topology
}

fn terminal_for_pane(ws: &Workspace, pane: PaneId) -> TerminalState {
    TerminalState::new(ws.terminal_id(pane).unwrap().clone(), "/tmp".into())
}

#[test]
fn workspace_rollup_all_unknown() {
    let topology = topology();
    let ws = &topology.workspaces[0];
    let terminal = terminal_for_pane(ws, ws.tabs[0].root_pane);
    let terminals = HashMap::from([(terminal.id.clone(), terminal)]);
    let (state, seen) = aggregate::aggregate_state(ws, &terminals);
    assert_eq!(state, AgentState::Unknown);
    assert!(seen);
}

fn add_tab(topology: &mut WorkspaceTopology<fn() -> u128>, name: &str) -> usize {
    topology
        .create_tab_with_runtime(0, |_, _| Ok(()))
        .unwrap()
        .unwrap();
    let ws = &mut topology.workspaces[0];
    let index = ws.tabs.len() - 1;
    ws.tabs[index].custom_name = Some(name.into());
    index
}
#[test]
fn workspace_rollup_priority() {
    let mut topology = topology();
    let target = topology.workspaces[0].tabs[0].root_pane;
    let id2 = topology
        .split_pane_with_runtime(
            0,
            maestro_workspace::layout::SplitRequest {
                target,
                direction: maestro_protocol::schema::SplitDirection::Right,
                ratio: None,
                focus_new_pane: true,
            },
            |_, _| Ok(()),
        )
        .unwrap()
        .unwrap()
        .0;
    let ws = &topology.workspaces[0];
    let root_id = ws.tabs[0]
        .panes
        .keys()
        .find(|id| **id != id2)
        .copied()
        .unwrap();
    let mut terminals = HashMap::new();
    let mut root_terminal = terminal_for_pane(ws, root_id);
    root_terminal.state = AgentState::Idle;
    terminals.insert(root_terminal.id.clone(), root_terminal);
    let mut second_terminal = terminal_for_pane(ws, id2);
    second_terminal.state = AgentState::Working;
    terminals.insert(second_terminal.id.clone(), second_terminal);

    let (state, seen) = aggregate::aggregate_state(ws, &terminals);

    assert_eq!(state, AgentState::Working);
    assert!(seen);
}

#[test]
fn workspace_rollup_done_unseen_beats_working() {
    let mut topology = topology();
    let target = topology.workspaces[0].tabs[0].root_pane;
    let id2 = topology
        .split_pane_with_runtime(
            0,
            maestro_workspace::layout::SplitRequest {
                target,
                direction: maestro_protocol::schema::SplitDirection::Right,
                ratio: None,
                focus_new_pane: true,
            },
            |_, _| Ok(()),
        )
        .unwrap()
        .unwrap()
        .0;
    let ws = &mut topology.workspaces[0];
    let root_id = ws.tabs[0]
        .panes
        .keys()
        .find(|id| **id != id2)
        .copied()
        .unwrap();
    let mut terminals = HashMap::new();
    let mut root_terminal = terminal_for_pane(ws, root_id);
    root_terminal.state = AgentState::Idle;
    terminals.insert(root_terminal.id.clone(), root_terminal);
    let mut second_terminal = terminal_for_pane(ws, id2);
    second_terminal.state = AgentState::Working;
    terminals.insert(second_terminal.id.clone(), second_terminal);
    let root = ws.tabs[0].panes.get_mut(&root_id).unwrap();
    root.seen = false;

    let (state, seen) = aggregate::aggregate_state(ws, &terminals);

    assert_eq!(state, AgentState::Idle);
    assert!(!seen);
}

#[test]
fn daemon_pane_details_prefers_agent_name_over_detected_agent_label() {
    let topology = topology();
    let ws = &topology.workspaces[0];
    let root_pane = ws.tabs[0].root_pane;
    let mut terminals = HashMap::new();
    let mut terminal = terminal_for_pane(ws, root_pane);
    terminal.detected_agent = Some(Agent::Pi);
    terminal.state = AgentState::Working;
    terminal.set_agent_name("planner".into());
    terminals.insert(terminal.id.clone(), terminal);

    let labels: Vec<_> = aggregate::pane_details(ws, &terminals)
        .into_iter()
        .map(|detail| (detail.label, detail.agent_label, detail.agent))
        .collect();

    assert_eq!(
        labels,
        vec![("planner".into(), "planner".into(), Some(Agent::Pi))]
    );
}

#[test]
fn daemon_pane_details_includes_tab_context_for_multi_tab_workspace() {
    let mut topology = topology();
    let second_tab = add_tab(&mut topology, "review");
    let ws = &mut topology.workspaces[0];
    ws.tabs[0].custom_name = Some("main".into());
    let root_pane = ws.tabs[0].root_pane;
    let review_pane = ws.tabs[second_tab].root_pane;
    let mut terminals = HashMap::new();
    let mut root_terminal = terminal_for_pane(ws, root_pane);
    root_terminal
        .set_hook_authority_at(
            maestro_daemon::terminal::state::HookReport {
                source: "test".into(),
                agent_label: "pi".into(),
                state: AgentState::Working,
                message: None,
                session_ref: None,
                seq: None,
            },
            std::time::Instant::now(),
        )
        .unwrap();
    terminals.insert(root_terminal.id.clone(), root_terminal);
    let mut review_terminal = terminal_for_pane(ws, review_pane);
    review_terminal
        .set_hook_authority_at(
            maestro_daemon::terminal::state::HookReport {
                source: "test".into(),
                agent_label: "claude".into(),
                state: AgentState::Idle,
                message: None,
                session_ref: None,
                seq: None,
            },
            std::time::Instant::now(),
        )
        .unwrap();
    terminals.insert(review_terminal.id.clone(), review_terminal);

    let labels: Vec<_> = aggregate::pane_details(ws, &terminals)
        .into_iter()
        .map(|detail| (detail.label, detail.agent_label, detail.agent))
        .collect();

    assert_eq!(
        labels,
        vec![
            ("main·pi".into(), "pi".into(), Some(Agent::Pi)),
            ("review·claude".into(), "claude".into(), Some(Agent::Claude)),
        ]
    );
}

#[test]
fn daemon_pane_details_use_tab_vector_index_not_stable_public_tab_number() {
    let mut topology = topology();
    let removed_tab = add_tab(&mut topology, "removed");
    let survivor_tab = add_tab(&mut topology, "survivor");
    let ws = &mut topology.workspaces[0];
    let survivor_pane = ws.tabs[survivor_tab].root_pane;
    assert!(ws.close_tab(removed_tab));

    let mut terminals = HashMap::new();
    let mut terminal = terminal_for_pane(ws, survivor_pane);
    terminal.detected_agent = Some(Agent::Codex);
    terminals.insert(terminal.id.clone(), terminal);

    let details = aggregate::pane_details(ws, &terminals);
    let survivor = details
        .iter()
        .find(|detail| detail.pane_id == survivor_pane)
        .expect("surviving tab agent should be listed");

    assert_eq!(ws.tabs[1].number, 3);
    assert_eq!(survivor.tab_idx, 1);
}
