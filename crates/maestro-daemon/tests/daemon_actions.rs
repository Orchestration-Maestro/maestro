//! Workspace reordering preserves logical navigation selection.
#![cfg(test)]
#![cfg(target_os = "linux")]

#[test]
fn daemon_waiting_sound_plays_even_in_active_workspace() {
    use maestro_daemon::state::actions::notification_sound_for_state_change;
    use maestro_detection::agents::AgentState;
    assert_eq!(
        notification_sound_for_state_change(true, AgentState::Working, AgentState::Blocked),
        Some(maestro_protocol::schema::NotificationShowSound::Request)
    );
}
#[test]
fn daemon_done_sound_only_plays_in_background() {
    use maestro_daemon::state::actions::notification_sound_for_state_change;
    use maestro_detection::agents::AgentState;
    assert_eq!(
        notification_sound_for_state_change(false, AgentState::Working, AgentState::Idle),
        Some(maestro_protocol::schema::NotificationShowSound::Done)
    );
    assert_eq!(
        notification_sound_for_state_change(true, AgentState::Working, AgentState::Idle),
        None
    );
    assert_eq!(
        notification_sound_for_state_change(false, AgentState::Unknown, AgentState::Idle),
        None
    );
}
#[test]
fn daemon_active_tab_suppression_preserves_unknown_focus_behavior() {
    use maestro_daemon::state::actions::active_tab_suppresses_notifications;
    assert!(active_tab_suppresses_notifications(true, None));
    assert!(active_tab_suppresses_notifications(true, Some(true)));
    assert!(!active_tab_suppresses_notifications(true, Some(false)));
    assert!(!active_tab_suppresses_notifications(false, None));
}
