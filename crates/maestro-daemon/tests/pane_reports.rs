//! Agent reports through the daemon's handler: state, session identity, authority and release.
#![cfg(target_os = "linux")]
use crate::support::api::{TestApp, daemon_with_workspaces, event_names, failure, pilot, request};
use maestro_detection::agents::AgentState;
use maestro_protocol::schema::{
    AgentSessionInfo, AgentStatus, ErrorBody, PaneInfo, ResponseResult,
};
use maestro_protocol::shared::AgentSessionRefKind;
use serde_json::{Value, json};

/// The session file the reports below name.
const SESSION: &str = "/sessions/pi-session.jsonl";

/// A daemon whose only pane runs a recognized Pi process that looks idle.
fn piloted() -> TestApp {
    let mut app = daemon_with_workspaces(&["/one"]);
    pilot(&mut app, AgentState::Idle);
    app
}

/// Makes the Pi integration report a session and then work, which anchors its authority.
fn anchored(app: &mut TestApp) {
    anchored_under(app, "maestro:pi");
}

/// Makes the Pi integration report a session and then work under the spelling `source`.
fn anchored_under(app: &mut TestApp, source: &str) {
    let session = json!({
        "type": "pane.report_agent_session", "pane_id": "w1:p1", "source": source,
        "agent": "pi", "agent_session_path": SESSION, "session_start_source": "startup",
        "seq": 1
    });
    accepted(app, session);
    let mut working = report(source, "pi", "working");
    working["message"] = json!("thinking");
    working["agent_session_path"] = json!(SESSION);
    working["seq"] = json!(2);
    accepted(app, working);
}

/// Answers `wire`, which must succeed with the plain acknowledgement.
fn accepted(app: &mut TestApp, wire: Value) {
    let text = wire.to_string();
    assert_eq!(request(app, wire), Ok(ResponseResult::Ok {}), "{text}");
}

/// The record of the first pane.
fn pane(app: &mut TestApp) -> PaneInfo {
    match request(app, json!({"type": "pane.get", "pane_id": "w1:p1"})) {
        Ok(ResponseResult::PaneInfo { pane }) => pane,
        other => panic!("expected a pane: {other:?}"),
    }
}

/// A state report for the first pane.
fn report(source: &str, agent: &str, state: &str) -> Value {
    json!({
        "type": "pane.report_agent", "pane_id": "w1:p1",
        "source": source, "agent": agent, "state": state
    })
}

/// The failure an unreadable agent label answers with.
fn blank_agent() -> Result<ResponseResult, ErrorBody> {
    failure("invalid_agent", "agent label must not be empty")
}

#[test]
fn a_custom_agent_report_becomes_the_effective_agent_and_state() {
    let mut app = daemon_with_workspaces(&["/one"]);
    accepted(&mut app, report("custom:hermes", "hermes", "working"));
    let reported = pane(&mut app);
    assert_eq!(reported.agent.as_deref(), Some("hermes"));
    assert_eq!(reported.agent_status, AgentStatus::Working);
}

#[test]
fn an_official_report_adds_its_session_identity_to_the_pane_record() {
    let mut app = piloted();
    anchored(&mut app);
    let reported = pane(&mut app);
    assert_eq!(reported.agent.as_deref(), Some("pi"));
    assert_eq!(reported.agent_status, AgentStatus::Working);
    assert_eq!(reported.agent_session, Some(pi_session()));
}

#[test]
fn a_session_report_alone_keeps_the_state_and_records_the_identity() {
    let mut app = piloted();
    let before = pane(&mut app);
    assert_eq!(before.agent_session, None);
    let session = json!({
        "type": "pane.report_agent_session", "pane_id": "w1:p1", "source": "maestro:pi",
        "agent": " Pi ", "agent_session_path": SESSION, "session_start_source": "startup",
        "seq": 1
    });
    accepted(&mut app, session);
    let after = pane(&mut app);
    assert_eq!(after.agent_status, before.agent_status);
    assert_eq!(
        after.agent_session.map(|session| session.value).as_deref(),
        Some(SESSION)
    );
}

#[test]
fn agent_labels_are_normalized_and_a_blank_one_is_refused() {
    let mut app = daemon_with_workspaces(&["/one"]);
    accepted(&mut app, report("custom:claude", " Claude-Code ", "idle"));
    assert_eq!(pane(&mut app).agent.as_deref(), Some("claude"));
    assert_eq!(
        request(&mut app, report("custom:x", "  ", "idle")),
        blank_agent()
    );
    let session = json!({
        "type": "pane.report_agent_session", "pane_id": "w1:p1", "source": "custom:x", "agent": ""
    });
    assert_eq!(request(&mut app, session), blank_agent());
    let release = json!({
        "type": "pane.release_agent", "pane_id": "w1:p1", "source": "custom:x", "agent": " "
    });
    assert_eq!(request(&mut app, release), blank_agent());
}

#[test]
fn a_report_for_a_missing_pane_is_refused() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let missing = failure("pane_not_found", "pane w1:p9 not found");
    for wire in [
        json!({"type": "pane.report_agent", "pane_id": "w1:p9", "source": "x", "agent": "x", "state": "idle"}),
        json!({"type": "pane.report_agent_session", "pane_id": "w1:p9", "source": "x", "agent": "x"}),
        json!({"type": "pane.report_metadata", "pane_id": "w1:p9", "source": "x", "title": "t"}),
        json!({"type": "pane.clear_agent_authority", "pane_id": "w1:p9"}),
        json!({"type": "pane.release_agent", "pane_id": "w1:p9", "source": "x", "agent": "x"}),
    ] {
        assert_eq!(request(&mut app, wire.clone()), missing, "{wire}");
    }
}

#[test]
fn a_report_publishes_the_agent_and_its_status() {
    let mut app = daemon_with_workspaces(&["/one"]);
    accepted(&mut app, report("custom:hermes", "hermes", "working"));
    assert_eq!(
        event_names(&app, 0),
        ["pane_agent_detected", "pane_agent_status_changed"]
    );
}

#[test]
fn clearing_authority_restores_the_detected_state() {
    let mut app = daemon_with_workspaces(&["/one"]);
    pilot(&mut app, AgentState::Working);
    anchored(&mut app);
    let mut blocked = report("maestro:pi", "pi", "blocked");
    blocked["seq"] = json!(3);
    accepted(&mut app, blocked);
    assert_eq!(pane(&mut app).agent_status, AgentStatus::Blocked);
    let clear = json!({
        "type": "pane.clear_agent_authority", "pane_id": "w1:p1", "source": "maestro:pi", "seq": 4
    });
    accepted(&mut app, clear);
    let restored = pane(&mut app);
    assert_eq!(restored.agent.as_deref(), Some("pi"));
    assert_eq!(restored.agent_status, AgentStatus::Working);
}

#[test]
fn releasing_a_custom_integration_clears_its_agent_but_an_official_release_keeps_the_process() {
    let mut app = daemon_with_workspaces(&["/one"]);
    accepted(&mut app, report("custom:hermes", "hermes", "working"));
    let release = json!({
        "type": "pane.release_agent", "pane_id": "w1:p1", "source": "custom:hermes",
        "agent": "hermes"
    });
    accepted(&mut app, release);
    assert_eq!(pane(&mut app).agent, None);
    let mut official = piloted();
    anchored(&mut official);
    let release = json!({
        "type": "pane.release_agent", "pane_id": "w1:p1", "source": "maestro:pi", "agent": "pi"
    });
    accepted(&mut official, release);
    let kept = pane(&mut official);
    assert_eq!(kept.agent.as_deref(), Some("pi"));
    assert_eq!(kept.agent_status, AgentStatus::Working);
}

#[test]
fn a_stale_sequence_is_ignored() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let mut fresh = report("custom:hermes", "hermes", "working");
    fresh["seq"] = json!(5);
    accepted(&mut app, fresh);
    let mut stale = report("custom:hermes", "hermes", "blocked");
    stale["seq"] = json!(3);
    accepted(&mut app, stale);
    assert_eq!(pane(&mut app).agent_status, AgentStatus::Working);
}

/// The session identity Pi's official integration reports, under its canonical source.
fn pi_session() -> AgentSessionInfo {
    AgentSessionInfo {
        source: "maestro:pi".into(),
        agent: "pi".into(),
        kind: AgentSessionRefKind::Path,
        value: SESSION.into(),
    }
}

#[test]
fn a_session_report_under_the_legacy_source_records_the_identity() {
    let mut app = piloted();
    let session = json!({
        "type": "pane.report_agent_session", "pane_id": "w1:p1", "source": "herdr:pi",
        "agent": "pi", "agent_session_path": SESSION, "session_start_source": "startup",
        "seq": 1
    });
    accepted(&mut app, session);
    assert_eq!(pane(&mut app).agent_session, Some(pi_session()));
}

#[test]
fn a_state_report_under_the_legacy_source_carries_the_identity() {
    let mut app = piloted();
    anchored_under(&mut app, "herdr:pi");
    let reported = pane(&mut app);
    assert_eq!(reported.agent_status, AgentStatus::Working);
    assert_eq!(reported.agent_session, Some(pi_session()));
}

#[test]
fn an_official_release_under_the_legacy_source_keeps_the_process() {
    let mut app = piloted();
    anchored(&mut app);
    let release = json!({
        "type": "pane.release_agent", "pane_id": "w1:p1", "source": "herdr:pi", "agent": "pi",
        "seq": 3
    });
    accepted(&mut app, release);
    let kept = pane(&mut app);
    assert_eq!(kept.agent.as_deref(), Some("pi"));
    assert_eq!(kept.agent_status, AgentStatus::Working);
}
