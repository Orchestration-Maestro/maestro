//! Supplied daemon roots and the shared socket selector.
#![cfg(all(test, unix))]

use maestro_daemon::paths::{DaemonPaths, action_path_component};

#[test]
fn daemon_path_table_covers_default_and_named_roots() {
    let root = "relative/./agent";
    for name in [None, Some("default"), Some("work")] {
        let paths = DaemonPaths::new(root, name).unwrap();
        let folder = if name == Some("work") {
            "relative/./agent/daemons/work"
        } else {
            root
        };
        assert_eq!(paths.agent_dir(), root);
        assert_eq!(paths.data_dir(), folder);
        assert_eq!(paths.api_socket_path(), format!("{folder}/daemon.sock"));
        assert_eq!(
            paths.client_socket_path(None, None, None),
            std::path::PathBuf::from(format!("{folder}/daemon-client.sock"))
        );
        assert_eq!(
            paths.snapshot_path(),
            format!("{folder}/daemon-snapshot.json")
        );
        assert_eq!(
            paths.history_path(),
            format!("{folder}/daemon-snapshot-history.json")
        );
        for (role, filename) in [
            ("in-process", "maestro-in-process.log"),
            ("client", "maestro-client.log"),
            ("daemon", "maestro-daemon.log"),
        ] {
            assert_eq!(paths.log_path(role), format!("{folder}/{filename}"));
        }
        assert_shared_paths(&paths, root);
    }
    for (root, socket) in [("", "daemon.sock"), ("agent/", "agent/daemon.sock")] {
        let paths = DaemonPaths::new(root, None).unwrap();
        assert_eq!(paths.api_socket_path(), socket);
    }
    assert!(!std::path::Path::new(root).exists());
    assert!(DaemonPaths::new(root, Some("../escape")).is_err());
}

/// Agent-wide resources remain shared by default and named daemons.
fn assert_shared_paths(paths: &DaemonPaths, root: &str) {
    assert_eq!(
        paths.detection_override(maestro_detection::agents::Agent::Codex),
        format!("{root}/detection/codex.toml")
    );
    assert_eq!(
        paths.detection_catalog(),
        format!("{root}/detection/catalog")
    );
    assert_eq!(
        paths.detection_status(),
        format!("{root}/detection/catalog/status.toml")
    );
    assert_eq!(
        paths.detection_manifest(maestro_detection::agents::Agent::Codex),
        format!("{root}/detection/catalog/remote/codex.toml")
    );
    assert_eq!(
        paths.action_config("example"),
        format!("{root}/workspace-actions/example/config")
    );
    assert_eq!(
        paths.action_state("example"),
        format!("{root}/workspace-actions/example/state")
    );
    assert_eq!(paths.announcements(), format!("{root}/announcements.json"));
    assert_eq!(paths.remote(), format!("{root}/remote"));
}

#[test]
fn daemon_paths_keep_api_and_client_stems_consistent() {
    use maestro_daemon::instances::{Selection, SelectionEnvironment};
    for (target, expected) in [(None, "agent"), (Some("work"), "agent/daemons/work")] {
        let selection =
            Selection::from_args("agent", &[], target, SelectionEnvironment::default()).unwrap();
        assert_eq!(selection.api_socket_path, format!("{expected}/daemon.sock"));
        assert_eq!(
            selection.client_socket_path,
            std::path::PathBuf::from(format!("{expected}/daemon-client.sock"))
        );
    }
    let selected = Selection::from_args(
        "agent",
        &[],
        Some("work"),
        SelectionEnvironment {
            api_socket: Some("api.sock"),
            client_socket: Some("client.sock"),
            daemon_name: Some("invalid/name"),
        },
    )
    .unwrap();
    assert_eq!(selected.api_socket_path, "agent/daemons/work/daemon.sock");
    assert_eq!(
        selected.client_socket_path.to_str(),
        Some("agent/daemons/work/daemon-client.sock")
    );
    let paths = DaemonPaths::new("agent", Some("work")).unwrap();
    assert_eq!(
        paths.client_socket_path(None, None, None).to_str(),
        Some("agent/daemons/work/daemon-client.sock")
    );
    assert_eq!(
        paths
            .client_socket_path(
                Some("chosen-client.sock"),
                Some("override.sock"),
                Some("other.sock")
            )
            .to_str(),
        Some("chosen-client.sock")
    );
    assert_eq!(
        paths
            .client_socket_path(None, Some("api.sock"), Some("other.sock"))
            .to_str(),
        Some("api-client.sock")
    );
    assert_eq!(
        paths
            .client_socket_path(None, None, Some("other.sock"))
            .to_str(),
        Some("other.sock")
    );
    assert_eq!(
        paths.client_socket_path(None, Some("/"), None).to_str(),
        Some("maestro-client.sock")
    );
}

#[test]
fn action_components_preserve_identity_and_bound_paths() {
    assert_eq!(
        action_path_component("examples.agent-telegram-notify"),
        "examples.agent-telegram-notify"
    );
    assert_eq!(action_path_component("example:a"), "example%3Aa");
    assert_eq!(action_path_component("Example"), "%45xample");
    assert_ne!(
        action_path_component("example:a"),
        action_path_component("example-a")
    );
    assert_ne!(
        action_path_component("Example"),
        action_path_component("example")
    );
    assert_ne!(
        action_path_component(&"A".repeat(120)),
        action_path_component(&"B".repeat(120))
    );
    assert!(action_path_component(&"A".repeat(120)).len() <= 120);
    assert_eq!(action_path_component("con"), "%con");
    assert_eq!(action_path_component("example."), "example%2E");
    assert_eq!(action_path_component(""), "%plugin");
    assert_eq!(action_path_component(".."), ".%2E");
    assert_eq!(action_path_component("../a%é"), "..%2Fa%25%C3%A9");
    assert_reserved_and_long_components();
    let paths = DaemonPaths::new("agent", None).unwrap();
    assert_eq!(
        paths.action_config("../a"),
        "agent/workspace-actions/..%2Fa/config"
    );
    assert_eq!(
        paths.action_state("../a"),
        "agent/workspace-actions/..%2Fa/state"
    );
}

/// Reserved stems and the long-component digest retain exact source outcomes.
fn assert_reserved_and_long_components() {
    for stem in ["con", "prn", "aux", "nul"] {
        assert_eq!(
            action_path_component(&format!("{stem}.txt")),
            format!("%{stem}.txt")
        );
    }
    for prefix in ["com", "lpt"] {
        for number in 1..=9 {
            let stem = format!("{prefix}{number}.txt");
            assert_eq!(action_path_component(&stem), format!("%{stem}"));
        }
    }
    assert_eq!(action_path_component("com0"), "com0");
    assert_eq!(action_path_component("lpt10"), "lpt10");
    let long = action_path_component(&"a".repeat(121));
    assert_eq!(long.len(), 120);
    assert_eq!(long, format!("{}-e9615320128c", "a".repeat(107)));
}

#[test]
fn action_encoding_keeps_extended_device_stems_out_of_the_reserved_domain() {
    for prefix in ["com", "lpt"] {
        for (superscript, encoded) in [('¹', "%C2%B9"), ('²', "%C2%B2"), ('³', "%C2%B3")] {
            let raw = format!("{prefix}{superscript}");
            assert!(maestro_path::win32::is_reserved(&raw));
            let component = action_path_component(&raw);
            assert_eq!(component, format!("{prefix}{encoded}"));
            assert!(component.is_ascii());
            assert!(!maestro_path::win32::is_reserved(&component));
        }
    }
}
