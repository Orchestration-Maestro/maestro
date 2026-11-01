//! Owned snapshots preserve topology, live terminal metadata and separate history.
#![cfg(test)]
#![cfg(target_os = "linux")]

use maestro_daemon::{
    app::state::DaemonState,
    persist::capture,
    settings::model::Config,
    terminal::{runtime_registry::TerminalRuntimeRegistry, state::TerminalState},
};
use maestro_workspace::{
    layout::{PaneId, SplitRequest},
    persist::snapshot::{LayoutSnapshot, SessionSnapshot},
    workspace::WorktreeSpaceMembership,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, RwLock},
};

/// State populated through the real allocator, with matching retained terminal metadata.
fn state(names: &[&str]) -> DaemonState<fn() -> u128> {
    let mut state = DaemonState::new(
        &Config::default(),
        (|| 0) as fn() -> u128,
        Arc::new(RwLock::new(
            maestro_detection::manifest::ManifestCache::new(None, None).unwrap(),
        )),
    );
    for name in names {
        let (pane, ()) = state
            .topology
            .new_with_tab(format!("/{name}").into(), None, |_, _| Ok(()))
            .unwrap();
        let workspace = state.topology.workspaces.last_mut().unwrap();
        workspace.custom_name = Some((*name).into());
        let id = workspace.terminal_id(pane).unwrap().clone();
        state.terminals.insert(
            id.clone(),
            TerminalState::new(id, format!("/{name}").into()),
        );
    }
    state.active = (!names.is_empty()).then_some(0);
    state
}

/// Captures metadata without supplying a live process owner.
fn snapshot(state: &DaemonState<fn() -> u128>) -> SessionSnapshot {
    capture(state, &TerminalRuntimeRegistry::new())
}

/// Splits the first tab with the allocator's transaction and matching metadata.
fn split(state: &mut DaemonState<fn() -> u128>) -> PaneId {
    let root = state.topology.workspaces[0].tabs[0].root_pane;
    let (pane, ()) = state
        .topology
        .split_pane_with_runtime(
            0,
            SplitRequest {
                target: root,
                direction: maestro_protocol::schema::SplitDirection::Right,
                ratio: None,
                focus_new_pane: true,
            },
            |_, _| Ok(()),
        )
        .unwrap()
        .unwrap();
    let id = state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    state
        .terminals
        .insert(id.clone(), TerminalState::new(id, "/split".into()));
    pane
}

/// Adds a tab using the same topology owner as native creation.
fn add_tab(state: &mut DaemonState<fn() -> u128>, name: Option<&str>) -> usize {
    state
        .topology
        .create_tab_with_runtime(0, |_, _| Ok(()))
        .unwrap()
        .unwrap();
    let index = state.topology.workspaces[0].tabs.len() - 1;
    state.topology.workspaces[0].tabs[index].custom_name = name.map(str::to_owned);
    index
}

#[test]
fn capture_keeps_workspace_order_active_and_selected() {
    let mut state = state(&["a", "b", "c"]);
    state.active = Some(1);
    state.selected = 2;
    assert!(state.move_workspace(1, 0));
    let saved = snapshot(&state);
    assert_eq!(
        saved
            .workspaces
            .iter()
            .map(|ws| ws.id.clone().unwrap())
            .collect::<Vec<_>>(),
        state
            .topology
            .workspaces
            .iter()
            .map(|ws| ws.id.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(saved.active, state.active);
    assert_eq!(saved.selected, state.selected);
}

#[test]
fn capture_keeps_workspace_and_tab_names_and_active_tab() {
    let mut state = state(&["one"]);
    state.topology.workspaces[0].custom_name = Some("renamed-workspace".into());
    let index = add_tab(&mut state, Some("logs"));
    state.topology.workspaces[0].active_tab = index;
    state.topology.workspaces[0].tabs[0].custom_name = Some("main".into());
    let saved = snapshot(&state);
    let workspace = &saved.workspaces[0];
    assert_eq!(workspace.custom_name.as_deref(), Some("renamed-workspace"));
    assert_eq!(workspace.active_tab, index);
    assert_eq!(workspace.tabs[0].custom_name.as_deref(), Some("main"));
    assert_eq!(workspace.tabs[1].custom_name.as_deref(), Some("logs"));
}

#[test]
fn capture_keeps_workspace_closure() {
    let mut state = state(&["one", "two"]);
    state.active = Some(1);
    state.selected = 1;
    state.close_selected_workspace();
    let saved = snapshot(&state);
    assert_eq!(saved.workspaces.len(), 1);
    assert_eq!(saved.workspaces[0].custom_name.as_deref(), Some("one"));
    assert_eq!(saved.active, Some(0));
    assert_eq!(saved.selected, 0);
}

#[test]
fn capture_keeps_sidebar_state() {
    let mut state = state(&["one"]);
    state.sidebar_width = 31;
    state.sidebar_section_split = 0.4;
    state.collapsed_space_keys.insert("repo-key".into());
    let saved = snapshot(&state);
    assert_eq!(saved.sidebar_width, Some(31));
    assert_eq!(saved.sidebar_section_split, Some(0.4));
    assert!(saved.collapsed_space_keys.contains("repo-key"));
}

#[test]
fn capture_keeps_worktree_space_membership() {
    let mut state = state(&["one"]);
    state.topology.workspaces[0].worktree_space = Some(WorktreeSpaceMembership {
        key: "repo-key".into(),
        label: "project".into(),
        repo_root: "/repo/project".into(),
        checkout_path: "/repo/project/branch".into(),
        is_linked_worktree: true,
    });
    assert_eq!(
        snapshot(&state).workspaces[0].worktree_space,
        state.topology.workspaces[0].worktree_space
    );
}

#[test]
fn capture_keeps_layout_focus_zoom_and_root_pane() {
    let mut state = state(&["one"]);
    let root = state.topology.workspaces[0].tabs[0].root_pane;
    let second = split(&mut state);
    state.topology.workspaces[0].tabs[0]
        .layout
        .focus_pane(second);
    state.toggle_zoom();
    let saved = snapshot(&state);
    let tab = &saved.workspaces[0].tabs[0];
    assert!(matches!(tab.layout, LayoutSnapshot::Split { .. }));
    assert_eq!(tab.focused, Some(second.raw()));
    assert_eq!(tab.root_pane, Some(root.raw()));
    assert!(tab.zoomed);
    assert_eq!(tab.panes.len(), 2);
}

#[test]
fn capture_keeps_focus_navigation() {
    let mut state = state(&["one"]);
    let root = state.topology.workspaces[0].tabs[0].root_pane;
    let second = split(&mut state);
    let layout = &mut state.topology.workspaces[0].tabs[0].layout;
    layout.focus_pane(root);
    let panes = layout.panes(maestro_workspace::layout::Rect::new(0, 0, 106, 20));
    let next = maestro_workspace::layout::find_in_direction(
        panes.iter().find(|pane| pane.id == root).unwrap(),
        maestro_workspace::layout::NavDirection::Right,
        &panes,
    )
    .unwrap();
    assert!(state.focus_pane_in_workspace(0, next));
    let saved = snapshot(&state);
    assert_eq!(saved.workspaces[0].tabs[0].focused, Some(second.raw()));
    assert_ne!(saved.workspaces[0].tabs[0].focused, Some(root.raw()));
}

#[test]
fn capture_keeps_resize_ratio_changes() {
    let mut state = state(&["one"]);
    let root = state.topology.workspaces[0].tabs[0].root_pane;
    split(&mut state);
    state.topology.workspaces[0].tabs[0].layout.focus_pane(root);
    let before = snapshot(&state);
    state.topology.workspaces[0].tabs[0].layout.resize_focused(
        maestro_workspace::layout::NavDirection::Right,
        0.05,
        maestro_workspace::layout::Rect::new(0, 0, 106, 20),
    );
    let after = snapshot(&state);
    let LayoutSnapshot::Split { ratio: first, .. } = before.workspaces[0].tabs[0].layout else {
        panic!("expected split")
    };
    let LayoutSnapshot::Split { ratio: second, .. } = after.workspaces[0].tabs[0].layout else {
        panic!("expected split")
    };
    assert!((first - second).abs() > f32::EPSILON);
}

#[test]
fn capture_keeps_tab_closure() {
    let mut state = state(&["one"]);
    let index = add_tab(&mut state, Some("logs"));
    state.topology.workspaces[0].active_tab = index;
    state.close_tab();
    let saved = snapshot(&state);
    assert_eq!(saved.workspaces[0].tabs.len(), 1);
    assert_eq!(saved.workspaces[0].active_tab, 0);
    assert!(saved.workspaces[0].tabs[0].custom_name.is_none());
}

#[test]
fn capture_keeps_pane_closure() {
    let mut state = state(&["one"]);
    split(&mut state);
    state.close_pane();
    let saved = snapshot(&state);
    let tab = &saved.workspaces[0].tabs[0];
    assert_eq!(tab.panes.len(), 1);
    assert!(matches!(tab.layout, LayoutSnapshot::Pane(_)));
    assert!(!tab.zoomed);
}

#[test]
fn capture_keeps_public_id_counters() {
    let mut state = state(&["one"]);
    let root = state.topology.workspaces[0].tabs[0].root_pane;
    let second = split(&mut state);
    let third = split(&mut state);
    let second_tab = add_tab(&mut state, None);
    state.topology.workspaces[0].close_pane(second);
    let saved = snapshot(&state);
    let workspace = &saved.workspaces[0];
    assert_eq!(
        workspace.public_pane_numbers,
        HashMap::from([
            (root.raw(), 1),
            (third.raw(), 3),
            (
                state.topology.workspaces[0].tabs[second_tab]
                    .root_pane
                    .raw(),
                4
            )
        ])
    );
    assert_eq!(workspace.next_public_pane_number, 5);
    assert_eq!(workspace.public_tab_numbers, vec![1, 2]);
    assert_eq!(workspace.next_public_tab_number, 3);
}

#[test]
fn capture_keeps_workspace_identity_and_pane_cwds() {
    let mut state = state(&["one"]);
    let root = state.topology.workspaces[0].tabs[0].root_pane;
    let second = split(&mut state);
    state.topology.workspaces[0].identity_cwd = "/stale".into();
    let first_id = state.topology.workspaces[0]
        .terminal_id(root)
        .unwrap()
        .clone();
    let second_id = state.topology.workspaces[0]
        .terminal_id(second)
        .unwrap()
        .clone();
    state.terminals.get_mut(&first_id).unwrap().cwd = "/current".into();
    state.terminals.get_mut(&second_id).unwrap().cwd = "/other".into();
    let saved = snapshot(&state);
    let workspace = &saved.workspaces[0];
    assert_eq!(workspace.identity_cwd, PathBuf::from("/current"));
    assert_eq!(
        workspace.tabs[0].panes[&root.raw()].cwd,
        PathBuf::from("/current")
    );
    assert_eq!(
        workspace.tabs[0].panes[&second.raw()].cwd,
        PathBuf::from("/other")
    );
}

#[test]
fn daemon_managed_agent_snapshot_omits_pending_and_persists_active_ownership() {
    use maestro_daemon::terminal::state::{DetectedStateUpdate, ManagedAgentLaunch};
    use maestro_detection::agents::{Agent, AgentState, AgentVisibility};
    let mut state = state(&["one"]);
    let root = state.topology.workspaces[0].tabs[0].root_pane;
    let id = state.topology.workspaces[0]
        .terminal_id(root)
        .unwrap()
        .clone();
    let now = std::time::Instant::now();
    state.terminals.get_mut(&id).unwrap().begin_managed_agent(
        ManagedAgentLaunch {
            name: "reviewer".into(),
            kind: Agent::Pi,
            settle_delay: std::time::Duration::ZERO,
            timeout: std::time::Duration::from_secs(1),
        },
        now,
    );
    let pending = snapshot(&state);
    let pane = &pending.workspaces[0].tabs[0].panes[&root.raw()];
    assert_eq!(pane.agent_name, None);
    assert_eq!(pane.managed_agent_kind, None);
    let terminal = state.terminals.get_mut(&id).unwrap();
    terminal.set_detected_state_with_screen_signals_at(
        DetectedStateUpdate {
            agent: Some(Agent::Pi),
            fallback_state: AgentState::Idle,
            process_exited: false,
            visible: AgentVisibility {
                visible_blocker: false,
                visible_idle: false,
                visible_working: false,
            },
        },
        now,
    );
    assert!(terminal.reconcile_managed_agent_at(now, false));
    let active = snapshot(&state);
    let pane = &active.workspaces[0].tabs[0].panes[&root.raw()];
    assert_eq!(pane.agent_name.as_deref(), Some("reviewer"));
    assert_eq!(pane.managed_agent_kind.as_deref(), Some("pi"));
}

#[test]
fn capture_keeps_hook_authority_agent_session() {
    use maestro_daemon::{
        agent_resume::{AgentSessionRef, AgentSessionRefKind, PersistedAgentSession},
        terminal::state::HookReport,
    };
    let mut state = state(&["one"]);
    let root = state.topology.workspaces[0].tabs[0].root_pane;
    let id = state.topology.workspaces[0]
        .terminal_id(root)
        .unwrap()
        .clone();
    state.handle_app_event(maestro_daemon::events::AppEvent::AgentProcessDetected {
        pane_id: root,
        agent: maestro_detection::agents::Agent::Pi,
        observed_at: std::time::Instant::now(),
    });
    let terminal = state.terminals.get_mut(&id).unwrap();
    terminal.set_persisted_agent_session(PersistedAgentSession {
        source: "maestro:pi".into(),
        agent: "pi".into(),
        session_ref: AgentSessionRef::path("/live.jsonl").unwrap(),
    });
    terminal
        .set_hook_authority_with_session_ref(HookReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            state: maestro_detection::agents::AgentState::Working,
            message: None,
            session_ref: AgentSessionRef::path("/live.jsonl"),
            seq: Some(20),
        })
        .unwrap();
    let saved = snapshot(&state);
    let session = saved.workspaces[0].tabs[0].panes[&root.raw()]
        .agent_session
        .as_ref()
        .unwrap();
    assert_eq!(session.source, "maestro:pi");
    assert_eq!(session.agent, "pi");
    assert_eq!(session.kind, AgentSessionRefKind::Path);
    assert_eq!(session.value, "/live.jsonl");
}

#[test]
fn capture_retains_restored_agent_session() {
    use maestro_daemon::agent_resume::{
        AgentSessionRef, AgentSessionRefKind, PersistedAgentSession,
    };
    let mut state = state(&["one"]);
    let root = state.topology.workspaces[0].tabs[0].root_pane;
    let id = state.topology.workspaces[0]
        .terminal_id(root)
        .unwrap()
        .clone();
    state
        .terminals
        .get_mut(&id)
        .unwrap()
        .set_persisted_agent_session(PersistedAgentSession {
            source: "maestro:opencode".into(),
            agent: "opencode".into(),
            session_ref: AgentSessionRef::id("restored-session").unwrap(),
        });
    let saved = snapshot(&state);
    let session = saved.workspaces[0].tabs[0].panes[&root.raw()]
        .agent_session
        .as_ref()
        .unwrap();
    assert_eq!(session.source, "maestro:opencode");
    assert_eq!(session.agent, "opencode");
    assert_eq!(session.kind, AgentSessionRefKind::Id);
    assert_eq!(session.value, "restored-session");
}

#[cfg(unix)]
use crate::support::launch;

#[test]
#[cfg(unix)]
fn setup_entry() {
    if maestro_process::pty::setup_entry().is_some() {
        std::process::exit(1);
    }
}

#[test]
#[cfg(unix)]
fn capture_keeps_pane_history_from_runtime() {
    with_history("capture_keeps_pane_history_from_runtime", 1);
}

#[test]
#[cfg(unix)]
fn capture_keeps_history_for_each_pane() {
    with_history("capture_keeps_history_for_each_pane", 2);
}

/// Owns each native process session inside the checked child runner.
#[cfg(unix)]
fn with_history(name: &str, count: usize) {
    if maestro_test_process::run_alone(name).unwrap() {
        return;
    }
    maestro_test_process::TestProcess::named("daemon_capture::setup_entry")
        .unwrap()
        .nocapture()
        .single_threaded()
        .invoke_with(|args| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(check_history(args, count));
            Ok::<_, std::io::Error>(())
        })
        .unwrap();
}

/// Uses an ordered cwd observation as readiness for already-ingested history bytes.
#[cfg(unix)]
async fn check_history(args: &[String], count: usize) {
    use maestro_daemon::{
        events::AppEvent,
        pane::{
            launch_env::PaneLaunchEnv,
            runtime::{AgentDetection, PaneRuntime},
        },
        persist::capture_history,
    };
    let mut state = state(&["one"]);
    if count == 2 {
        split(&mut state);
    }
    let (launch, mut receiver) = launch::launch_inputs(args, Arc::clone(&state.manifests));
    let mut runtimes = TerminalRuntimeRegistry::new();
    let panes = state.topology.workspaces[0].tabs[0].layout.pane_ids();
    let markers = if count == 1 {
        vec!["alpha\r\nbeta\r\ngamma"]
    } else {
        vec!["first-pane-history", "second-pane-history"]
    };
    for (pane, marker) in panes.iter().zip(&markers) {
        let mut config = launch.runtime_config(&Config::default(), *pane, (3, 20));
        config.agent_detection = AgentDetection::Disabled;
        let command = format!("printf '{marker}\\r\\n\\033]7;file:///tmp\\033\\\\'; read -r line");
        let runtime = PaneRuntime::spawn_shell_command(
            config,
            &command,
            std::env::current_dir().unwrap(),
            &PaneLaunchEnv::default(),
        )
        .unwrap();
        let id = state.topology.workspaces[0]
            .terminal_id(*pane)
            .unwrap()
            .clone();
        runtimes.insert(id, runtime);
        loop {
            if let AppEvent::TerminalCwdReported { pane_id, .. } = receiver.recv().await.unwrap() {
                assert_eq!(pane_id, *pane);
                break;
            }
        }
    }
    let encoded = serde_json::to_string(&capture(&state, &runtimes)).unwrap();
    assert!(!encoded.contains("\"history\""));
    let history = capture_history(&state.topology.workspaces, &runtimes);
    assert_history(&encoded, &history, &panes, &markers);
    for (_, runtime) in runtimes.drain_for_handoff() {
        runtime.shutdown().await;
    }
}

/// Checks each saved pane against its own ingested history, never structural JSON.
#[cfg(unix)]
fn assert_history(
    encoded: &str,
    history: &maestro_workspace::persist::snapshot::SessionHistorySnapshot,
    panes: &[PaneId],
    markers: &[&str],
) {
    for (pane, marker) in panes.iter().zip(markers.iter()) {
        let saved = &history.workspaces[0].tabs[0].panes[&pane.raw()];
        if markers.len() == 1 {
            assert!(!encoded.contains("alpha"));
            assert!(saved.ansi.contains("alpha"));
            assert!(saved.ansi.contains("gamma"));
            assert!(saved.lines >= 3);
        } else {
            assert!(!encoded.contains(*marker));
            assert!(saved.ansi.contains(*marker));
        }
    }
}
