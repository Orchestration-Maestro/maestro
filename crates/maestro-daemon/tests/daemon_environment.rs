//! Environment names consumed by pane launches, workspace actions and daemon selection.
#![cfg(test)]
use maestro_daemon::environment::{Consumer, NAMES};

#[test]
fn daemon_environment_names_match_their_consumers() {
    unique_names();
    for (consumer, expected) in [
        (
            Consumer::Pane,
            vec![
                "MAESTRO_ENV",
                "MAESTRO_SOCKET_PATH",
                "MAESTRO_BIN_PATH",
                "MAESTRO_WORKSPACE_ID",
                "MAESTRO_TAB_ID",
                "MAESTRO_PANE_ID",
                "MAESTRO_PANE_RUNTIME_ID",
                "MAESTRO_ACTION_ROOT",
                "MAESTRO_ACTION_CONFIG_DIR",
                "MAESTRO_ACTION_STATE_DIR",
                "MAESTRO_ACTION_MANIFEST_ID",
                "MAESTRO_ACTION_ENTRYPOINT_ID",
                "MAESTRO_ACTION_CONTEXT_JSON",
            ],
        ),
        (
            Consumer::Action,
            vec![
                "MAESTRO_ENV",
                "MAESTRO_SOCKET_PATH",
                "MAESTRO_BIN_PATH",
                "MAESTRO_WORKSPACE_ID",
                "MAESTRO_TAB_ID",
                "MAESTRO_PANE_ID",
                "MAESTRO_ACTION_ROOT",
                "MAESTRO_ACTION_CONFIG_DIR",
                "MAESTRO_ACTION_STATE_DIR",
                "MAESTRO_ACTION_MANIFEST_ID",
                "MAESTRO_ACTION_ENTRYPOINT_ID",
                "MAESTRO_ACTION_CONTEXT_JSON",
                "MAESTRO_ACTION_ID",
                "MAESTRO_ACTION_EVENT",
                "MAESTRO_ACTION_EVENT_JSON",
                "MAESTRO_ACTION_CLICKED_URL",
                "MAESTRO_ACTION_LINK_HANDLER_ID",
            ],
        ),
        (
            Consumer::DaemonSelection,
            vec![
                "MAESTRO_DAEMON",
                "MAESTRO_SOCKET_PATH",
                "MAESTRO_CLIENT_SOCKET_PATH",
            ],
        ),
    ] {
        let actual = NAMES
            .iter()
            .filter(|e| e.consumers.contains(&consumer))
            .map(|e| e.name)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(actual, expected.into_iter().collect());
    }
}

/// Every published name has a single row, including the existing selection owner.
fn unique_names() {
    let names = NAMES
        .iter()
        .map(|entry| entry.name)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(names.len(), NAMES.len());
    assert!(names.contains(maestro_daemon::instances::SESSION_ENV_VAR));
    assert_eq!(NAMES.len(), 20);
}
