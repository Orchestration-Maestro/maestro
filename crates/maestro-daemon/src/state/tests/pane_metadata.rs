//! Pane metadata reports through the daemon's handler: validation, tokens, presentation,
//! sequences and publication.
use crate::state::App;
use crate::state::test_support::api::{
    accepted, daemon_with_workspaces, detect, pane_record, root_pane,
};
use crate::state::test_support::pane_metadata::{report, titled, tokens};
use crate::terminal::model::{DetectedStateUpdate, TerminalState};
use maestro_detection::agents::{Agent, AgentState, AgentVisibility};
use serde_json::json;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
/// The retained state of the first pane's terminal.
fn terminal_of(app: &mut App) -> &TerminalState {
    let root = root_pane(app, 0).unwrap();
    let id = app.state.topology.workspaces[0].terminal_id(root).unwrap();
    &app.state.terminals[id]
}
/// The detection of `agent` in `state`, reporting a process exit when `process_exited`.
fn update(agent: Option<Agent>, state: AgentState, process_exited: bool) -> DetectedStateUpdate {
    DetectedStateUpdate {
        agent,
        fallback_state: state,
        visible: AgentVisibility::default(),
        process_exited,
    }
}
/// Feeds the first pane's terminal a detection of `update` at `at`.
fn observe(app: &mut App, update: DetectedStateUpdate, at: Instant) {
    let root = root_pane(app, 0).unwrap();
    let terminal = app.state.topology.workspaces[0]
        .terminal_id(root)
        .unwrap()
        .clone();
    app.state
        .terminals
        .get_mut(&terminal)
        .unwrap()
        .set_detected_state_with_screen_signals_at(update, at);
}
#[test]
fn tokens_patch_and_clear_without_touching_the_presentation() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let set = report(
        "user:metadata.test-1",
        json!({"tokens": {"summary": "reviewing auth", "model": "opus"}}),
    )
    .unwrap();
    accepted(&mut app, set).unwrap();
    let expected = |pairs: &[(&str, &str)]| {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect::<HashMap<_, _>>()
    };
    assert_eq!(
        tokens(&mut app).unwrap(),
        expected(&[("summary", "reviewing auth"), ("model", "opus")])
    );
    let patch = report(
        "user:metadata.test-1",
        json!({"tokens": {"summary": "done", "model": null}}),
    )
    .unwrap();
    accepted(&mut app, patch).unwrap();
    assert_eq!(tokens(&mut app).unwrap(), expected(&[("summary", "done")]));
    assert!(terminal_of(&mut app).agent_metadata.is_empty());
}
#[test]
fn a_report_ignored_after_the_process_exit_does_not_advance_the_sequence() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    detect(&mut app, Agent::Pi, AgentState::Idle);
    let initial =
        titled(json!({"source": "custom:pi-metadata", "agent": "pi", "seq": 100})).unwrap();
    accepted(&mut app, initial).unwrap();
    let initial_tokens = report(
        "custom:pi-tokens",
        json!({"agent": "pi", "tokens": {"generation": "old"}, "seq": 100}),
    )
    .unwrap();
    accepted(&mut app, initial_tokens).unwrap();
    let exit_at = Instant::now() + Duration::from_millis(1);
    observe(
        &mut app,
        update(Some(Agent::Pi), AgentState::Idle, true),
        exit_at,
    );
    let gone = update(None, AgentState::Unknown, false);
    observe(&mut app, gone, exit_at + Duration::from_millis(1));
    let stale = titled(json!({
        "source": "custom:pi-metadata", "agent": "pi", "title": "stale", "seq": 200
    }))
    .unwrap();
    accepted(&mut app, stale).unwrap();
    accepted(
        &mut app,
        titled(json!({"source": "maestro:pi", "seq": 200})).unwrap(),
    )
    .unwrap();
    let back = update(Some(Agent::Pi), AgentState::Idle, false);
    observe(&mut app, back, exit_at + Duration::from_millis(2));
    let fresh = titled(json!({
        "source": "custom:pi-metadata", "agent": "pi", "title": "fresh", "seq": 1
    }))
    .unwrap();
    accepted(&mut app, fresh).unwrap();
    let fresh_tokens = report(
        "custom:pi-tokens",
        json!({"agent": "pi", "tokens": {"generation": "new"}, "seq": 1}),
    )
    .unwrap();
    accepted(&mut app, fresh_tokens).unwrap();
    let retained = terminal_of(&mut app);
    let title = retained.agent_metadata["custom:pi-metadata"]
        .title
        .as_deref();
    assert_eq!(title, Some("fresh"));
    let generation = retained.metadata_tokens.values().get("generation").cloned();
    assert_eq!(generation.as_deref(), Some("new"));
}
#[test]
fn legacy_and_canonical_source_spellings_share_one_sequence() {
    for (fresh, stale) in [
        ("maestro:claude", "herdr:claude"),
        ("herdr:claude", "maestro:claude"),
    ] {
        let mut app = daemon_with_workspaces(&["/one"]).unwrap();
        detect(&mut app, Agent::Claude, AgentState::Idle);
        let newest = report(
            fresh,
            json!({"title": "new", "tokens": {"summary": "new"}, "seq": 10}),
        )
        .unwrap();
        accepted(&mut app, newest).unwrap();
        let older = report(
            stale,
            json!({"title": "old", "tokens": {"summary": "old"}, "seq": 9}),
        )
        .unwrap();
        accepted(&mut app, older).unwrap();
        let kept = pane_record(&mut app, "w1:p1").unwrap();
        assert_eq!(kept.title.as_deref(), Some("new"), "{fresh} then {stale}");
        assert_eq!(kept.tokens["summary"], "new", "{fresh} then {stale}");
        accepted(
            &mut app,
            report(stale, json!({"title": "newer", "seq": 11})).unwrap(),
        )
        .unwrap();
        assert_eq!(
            pane_record(&mut app, "w1:p1").unwrap().title.as_deref(),
            Some("newer")
        );
        let sources: Vec<_> = terminal_of(&mut app).agent_metadata.keys().collect();
        assert_eq!(sources, ["maestro:claude"], "{fresh} then {stale}");
    }
}
#[test]
fn a_legacy_guard_spelling_is_ignored_after_the_process_exit_without_advancing_the_sequence() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    detect(&mut app, Agent::Claude, AgentState::Idle);
    let exit_at = Instant::now() + Duration::from_millis(1);
    let exited = update(Some(Agent::Claude), AgentState::Idle, true);
    observe(&mut app, exited, exit_at);
    let gone = update(None, AgentState::Unknown, false);
    observe(&mut app, gone, exit_at + Duration::from_millis(1));
    let stale = titled(json!({
        "source": "custom:meta", "applies_to_source": "herdr:claude", "title": "stale", "seq": 200
    }))
    .unwrap();
    accepted(&mut app, stale).unwrap();
    let back = update(Some(Agent::Claude), AgentState::Idle, false);
    observe(&mut app, back, exit_at + Duration::from_millis(2));
    let fresh = titled(json!({
        "source": "custom:meta", "applies_to_source": "maestro:claude", "title": "fresh", "seq": 1
    }))
    .unwrap();
    accepted(&mut app, fresh).unwrap();
    let title = terminal_of(&mut app).agent_metadata["custom:meta"]
        .title
        .as_deref();
    assert_eq!(title, Some("fresh"));
}
