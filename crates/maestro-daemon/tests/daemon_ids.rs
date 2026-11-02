//! Stable pane numbers and historical aliases resolve only live panes.
#![cfg(test)]
#![cfg(target_os = "linux")]

#[cfg(unix)]
use crate::support::launch;

use maestro_daemon::{app::state::DaemonState, settings::model::Config};
#[cfg(unix)]
use maestro_daemon::{
    app::{
        App,
        terminal_targets::{TerminalTargetCandidate, TerminalTargetError},
    },
    terminal::state::TerminalState,
};
#[cfg(unix)]
use maestro_detection::agents::{Agent, AgentState};
#[cfg(unix)]
use maestro_protocol::schema::AgentStatus;
#[cfg(unix)]
use maestro_workspace::{layout::PaneId, terminal_id::TerminalId};
#[cfg(unix)]
use std::sync::{Arc, RwLock};
#[cfg(unix)]
type TargetApp = App<fn() -> u128>;

#[test]
fn public_target_forms_keep_alias_and_number_meaning() {
    let mut state = DaemonState::new(
        &Config::default(),
        || 0,
        std::sync::Arc::new(std::sync::RwLock::new(
            maestro_detection::manifest::ManifestCache::new(None, None).unwrap(),
        )),
    );
    let (removed, ()) = state
        .topology
        .new_with_tab("/first".into(), None, |_, _| Ok(()))
        .unwrap();
    state
        .topology
        .new_with_tab("/second".into(), None, |_, _| Ok(()))
        .unwrap();
    let (pane, ()) = state
        .topology
        .create_tab_with_runtime(0, |_, _| Ok(()))
        .unwrap()
        .unwrap();
    assert!(state.topology.workspaces[0].close_tab(0));
    assert_eq!(pane.raw(), 3);
    assert_eq!(state.public_pane_id(0, pane).as_deref(), Some("w1:p2"));
    assert_eq!(state.public_tab_id(0, 0).as_deref(), Some("w1:t2"));
    for spelling in ["w1:p2", "w1-2", "p_3", "p_w1_3"] {
        assert_eq!(state.parse_pane_id(spelling), Some((0, pane)));
    }
    assert_eq!(state.parse_pane_id("w1:p1"), None);
    assert_eq!(state.parse_pane_id("p_w2_3"), None);
    state.pane_id_aliases.insert(99, pane);
    state.public_pane_id_aliases.insert("old-pane".into(), pane);
    assert_eq!(state.parse_pane_id("p_99"), Some((0, pane)));
    assert_eq!(state.parse_pane_id("old-pane"), Some((0, pane)));
    assert_eq!(state.parse_current_public_pane_id("p_99"), None);
    assert_eq!(state.parse_current_public_pane_id("w1:p2"), Some((0, pane)));
    state.pane_id_aliases.insert(pane.raw(), removed);
    state
        .public_pane_id_aliases
        .insert("old-pane".into(), removed);
    assert_eq!(state.parse_pane_id("p_3"), None);
    assert_eq!(state.parse_pane_id("old-pane"), None);
    #[cfg(unix)]
    assert_terminal_target_forms();
}

#[test]
fn workspace_and_tab_spellings_keep_positions_distinct_from_public_numbers() {
    let mut state = DaemonState::new(
        &Config::default(),
        || 0,
        std::sync::Arc::new(std::sync::RwLock::new(
            maestro_detection::manifest::ManifestCache::new(None, None).unwrap(),
        )),
    );
    for cwd in ["/first", "/second"] {
        state
            .topology
            .new_with_tab(cwd.into(), None, |_, _| Ok(()))
            .unwrap();
    }
    state
        .topology
        .create_tab_with_runtime(0, |_, _| Ok(()))
        .unwrap()
        .unwrap();
    assert!(state.topology.workspaces[0].close_tab(0));
    state.topology.workspaces.swap(0, 1);
    assert_eq!(state.parse_workspace_id("w1"), Some(1));
    for form in ["w_1", "1", "w2"] {
        assert_eq!(state.parse_workspace_id(form), Some(0));
    }
    for form in ["w1:t2", "w1:1", "t_w1_1", "t_w_2_1", "2:1"] {
        assert_eq!(state.parse_tab_id(form), Some((1, 0)), "{form}");
    }
    for form in ["", "0", "w_0", "w999", "w_-1", "w_1x"] {
        assert_eq!(state.parse_workspace_id(form), None, "{form}");
    }
    for form in [
        "w1:t1",
        "w1:t0",
        "w1:t!",
        "w1:0",
        "w1:99",
        "t_w1_0",
        "w999:t1",
        "999:1",
        "w1:t2:extra",
    ] {
        assert_eq!(state.parse_tab_id(form), None, "{form}");
    }
    for form in [
        "w1:p0", "w1:p!", "w1-0", "p_0", "p_w1_0", "w999:p1", "999-1", "p_x", "p_999999", "missing",
    ] {
        assert_eq!(state.parse_pane_id(form), None, "{form}");
    }
}

/// Builds noncontiguous public identities with no native process.
#[cfg(unix)]
fn target_app() -> (TargetApp, PaneId, TerminalId) {
    let manifests = Arc::new(RwLock::new(
        maestro_detection::manifest::ManifestCache::new(None, None).unwrap(),
    ));
    let (launch, receiver) = launch::launch_inputs(&[], manifests);
    let mut app = App::new(
        &Config::default(),
        (|| 0) as fn() -> u128,
        launch,
        receiver,
        maestro_daemon::api::event_hub::EventHub::default(),
    );
    let (removed, ()) = app
        .state
        .topology
        .new_with_tab("/first".into(), None, |_, _| Ok(()))
        .unwrap();
    let (pane, ()) = app
        .state
        .topology
        .create_tab_with_runtime(0, |_, _| Ok(()))
        .unwrap()
        .unwrap();
    assert!(app.state.topology.workspaces[0].close_tab(0));
    let id = app.state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    let mut terminal = TerminalState::new(id.clone(), "/first".into());
    terminal.detected_agent = Some(Agent::Codex);
    terminal.state = AgentState::Idle;
    app.state.terminals.insert(id.clone(), terminal);
    app.state.topology.workspaces[0]
        .pane_state_mut(pane)
        .unwrap()
        .seen = false;
    app.state.pane_id_aliases.insert(99, pane);
    app.state
        .public_pane_id_aliases
        .insert("old-pane".into(), pane);
    assert!(
        app.state.topology.workspaces[0]
            .pane_state(removed)
            .is_none()
    );
    (app, pane, id)
}

/// Current-only targeting preserves precedence, alias rejection and viewport ambiguity.
#[cfg(unix)]
fn assert_terminal_target_forms() {
    let (mut app, pane, id) = target_app();
    assert_current_target_forms(&mut app, pane, &id);
    let (other, other_id) = assert_target_collisions(&mut app, pane, &id);
    assert_target_ambiguity(&mut app, other, &id, &other_id);
}

/// Current handles resolve while historical spellings are accepted only as names.
#[cfg(unix)]
fn assert_current_target_forms(app: &mut TargetApp, pane: PaneId, id: &TerminalId) {
    for form in [
        "w1:p1", "w1:t2", "p_99", "old-pane", "p_2", "p_w1_2", "w1-2", "w1:p0", "w1:p!", "",
    ] {
        let expected = TerminalTargetError::NotFound {
            target: form.into(),
        };
        assert_eq!(
            app.resolve_terminal_target(form).unwrap_err(),
            expected,
            "{form}"
        );
        assert_eq!(
            app.resolve_agent_target(form).unwrap_err(),
            expected,
            "{form}"
        );
    }
    for form in [id.to_string(), "w1:p2".into(), "codex".into()] {
        let resolved = app.resolve_terminal_target(&form).unwrap();
        assert_eq!(
            (
                resolved.ws_idx,
                resolved.tab_idx,
                resolved.pane_id,
                resolved.terminal_id
            ),
            (0, 0, pane, id.to_string())
        );
    }
    assert_eq!(app.resolve_agent_target("w1:p2").unwrap().pane_id, pane);
    for form in [id.to_string(), "codex".into()] {
        assert_eq!(
            app.resolve_agent_target(&form).unwrap_err(),
            TerminalTargetError::NotFound { target: form }
        );
    }
    // Historical forms may still be explicit names, never parsed aliases.
    for form in ["old-pane", "p_99", "p_w1_2", "w1-2"] {
        app.state
            .terminals
            .get_mut(id)
            .unwrap()
            .set_agent_name(form.into());
        assert_eq!(app.resolve_terminal_target(form).unwrap().pane_id, pane);
        assert_eq!(app.resolve_agent_target(form).unwrap().pane_id, pane);
    }
    app.state
        .terminals
        .get_mut(id)
        .unwrap()
        .set_agent_name(String::new());
}

/// Terminal handles precede names and current pane handles precede names for both routes.
#[cfg(unix)]
fn assert_target_collisions(
    app: &mut TargetApp,
    pane: PaneId,
    id: &TerminalId,
) -> (PaneId, TerminalId) {
    let (other, ()) = app
        .state
        .topology
        .new_with_tab("/other".into(), None, |_, _| Ok(()))
        .unwrap();
    let other_id = app.state.topology.workspaces[1]
        .terminal_id(other)
        .unwrap()
        .clone();
    let mut other_terminal = TerminalState::new(other_id.clone(), "/other".into());
    other_terminal.set_agent_name(id.to_string());
    app.state.terminals.insert(other_id.clone(), other_terminal);
    assert_eq!(
        app.resolve_terminal_target(&id.to_string())
            .unwrap()
            .pane_id,
        pane
    );
    assert_eq!(
        app.resolve_agent_target(&id.to_string()).unwrap().pane_id,
        other
    );
    app.state
        .terminals
        .get_mut(&other_id)
        .unwrap()
        .set_agent_name("w1:p2".into());
    assert_eq!(app.resolve_terminal_target("w1:p2").unwrap().pane_id, pane);
    assert_eq!(app.resolve_agent_target("w1:p2").unwrap().pane_id, pane);
    app.state.topology.workspaces.swap(0, 1);
    assert_eq!(app.resolve_terminal_target("w1:p2").unwrap().ws_idx, 1);
    (other, other_id)
}

/// Ambiguity retains every ordered viewport with its exact public observations.
#[cfg(unix)]
fn assert_target_ambiguity(
    app: &mut TargetApp,
    other: PaneId,
    id: &TerminalId,
    other_id: &TerminalId,
) {
    for terminal in app.state.terminals.values_mut() {
        terminal.set_agent_name("duplicate".into());
    }
    let expected = TerminalTargetError::Ambiguous {
        target: "duplicate".into(),
        candidates: vec![
            TerminalTargetCandidate {
                terminal_id: other_id.to_string(),
                pane_id: "w2:p1".into(),
                workspace_id: "w2".into(),
                tab_id: "w2:t1".into(),
                cwd: Some("/other".into()),
                agent_status: AgentStatus::Unknown,
            },
            TerminalTargetCandidate {
                terminal_id: id.to_string(),
                pane_id: "w1:p2".into(),
                workspace_id: "w1".into(),
                tab_id: "w1:t2".into(),
                cwd: Some("/first".into()),
                agent_status: AgentStatus::Done,
            },
        ],
    };
    assert_eq!(
        app.resolve_terminal_target("duplicate").unwrap_err(),
        expected
    );
    assert_eq!(app.resolve_agent_target("duplicate").unwrap_err(), expected);
    // The same terminal attached twice is ambiguous by viewport, not deduplicated.
    app.state.topology.workspaces[0]
        .pane_state_mut(other)
        .unwrap()
        .attached_terminal_id = id.clone();
    let TerminalTargetError::Ambiguous { target, candidates } =
        app.resolve_terminal_target(&id.to_string()).unwrap_err()
    else {
        panic!("duplicate attachments must be ambiguous")
    };
    assert_eq!(target, id.to_string());
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].pane_id, "w2:p1");
    assert_eq!(candidates[1].pane_id, "w1:p2");
}
