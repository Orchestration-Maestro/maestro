//! Owned target selection without process-global state.
use super::*;

#[cfg(unix)]
fn select(args: &[&str], env: SelectionEnvironment<'_>) -> Selection {
    let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    let original = args.clone();
    let selection = Selection::from_args("agent", &args, None, env).unwrap();
    assert_eq!(args, original);
    selection
}

#[test]
#[cfg(unix)]
fn attach_daemon_option_selects_an_owned_name() {
    let selected = select(
        &["maestro", "attach", "--daemon", "work", "--session", "chat"],
        SelectionEnvironment::default(),
    );
    assert_eq!(selected.paths.name(), Some("work"));
    assert!(selected.explicit);
}
#[test]
#[cfg(unix)]
fn attach_daemon_equals_form_selects_the_same_name() {
    let selected = select(
        &["maestro", "attach", "--daemon=work"],
        SelectionEnvironment::default(),
    );
    assert_eq!(selected.paths.name(), Some("work"));
    assert!(selected.explicit);
}
#[test]
#[cfg(unix)]
fn child_session_option_remains_after_separator() {
    let selected = select(
        &[
            "maestro",
            "attach",
            "--",
            "--session",
            "child",
            "--daemon",
            "unused",
        ],
        SelectionEnvironment::default(),
    );
    assert_eq!(selected.paths.name(), None);
    assert!(!selected.explicit);
}
#[test]
#[cfg(unix)]
fn child_session_equals_form_remains_after_separator() {
    let selected = select(
        &[
            "maestro",
            "attach",
            "--",
            "--session=child",
            "--daemon=unused",
        ],
        SelectionEnvironment::default(),
    );
    assert_eq!(selected.paths.name(), None);
}
#[test]
#[cfg(unix)]
fn named_attach_selects_without_rewriting_dispatch() {
    let selected = select(
        &["maestro", "attach", "--daemon", "work"],
        SelectionEnvironment::default(),
    );
    assert_eq!(selected.paths.data_dir(), "agent/daemons/work");
    let named_help = select(
        &["maestro", "attach", "--daemon", "help", "--session", "help"],
        SelectionEnvironment::default(),
    );
    assert_eq!(named_help.paths.name(), Some("help"));
}
#[test]
#[cfg(unix)]
fn attach_help_remains_for_command_dispatch() {
    for help in ["help", "--help", "-h"] {
        let selected = select(
            &["maestro", "attach", "--daemon", "bad/name", help],
            SelectionEnvironment {
                daemon_name: Some("invalid/name"),
                ..Default::default()
            },
        );
        assert!(!selected.explicit);
        assert_eq!(selected.paths.name(), None);
    }
}
#[test]
#[cfg(unix)]
fn explicit_default_daemon_uses_root_paths() {
    let selected = select(
        &["maestro", "attach", "--daemon=default"],
        SelectionEnvironment::default(),
    );
    assert!(selected.explicit);
    assert_eq!(selected.paths.data_dir(), "agent");
}
#[test]
#[cfg(unix)]
fn inherited_daemon_name_is_not_explicit() {
    let selected = select(
        &["maestro"],
        SelectionEnvironment {
            daemon_name: Some("work"),
            ..Default::default()
        },
    );
    assert!(!selected.explicit);
    assert_eq!(selected.paths.name(), Some("work"));
}
#[test]
#[cfg(unix)]
fn inherited_default_daemon_uses_root_paths() {
    let selected = select(
        &["maestro"],
        SelectionEnvironment {
            daemon_name: Some("default"),
            ..Default::default()
        },
    );
    assert!(!selected.explicit);
    assert_eq!(selected.paths.data_dir(), "agent");
}
#[test]
fn default_attach_guidance_uses_attach_command() {
    assert_eq!(
        DaemonPaths::new("agent", None)
            .unwrap()
            .local_attach_command(),
        "maestro attach"
    );
}
#[test]
fn named_attach_guidance_includes_daemon_name() {
    assert_eq!(
        DaemonPaths::new("agent", Some("work"))
            .unwrap()
            .local_attach_command(),
        "maestro attach --daemon work"
    );
}
#[test]
fn default_stop_guidance_uses_daemon_command() {
    assert_eq!(
        DaemonPaths::new("agent", None)
            .unwrap()
            .local_stop_command(),
        "maestro daemon stop"
    );
}
#[test]
fn named_stop_guidance_includes_daemon_name() {
    assert_eq!(
        DaemonPaths::new("agent", Some("work"))
            .unwrap()
            .local_stop_command(),
        "maestro daemon stop work"
    );
}
#[test]
#[cfg(unix)]
fn restart_guidance_keeps_stop_warning_and_reattach() {
    let selection = select(
        &["maestro", "attach", "--daemon=work"],
        SelectionEnvironment::default(),
    );
    assert_eq!(
        selection.restart_after_update_guidance(),
        "Stop the old daemon to use the new version.\nStopping exits pane processes.\nRun `maestro daemon stop work`, then run `maestro attach --daemon work` again."
    );
}
#[test]
#[cfg(unix)]
fn restart_guidance_keeps_socket_override() {
    let selection = select(
        &["maestro"],
        SelectionEnvironment {
            api_socket: Some("custom.sock"),
            ..Default::default()
        },
    );
    assert_eq!(
        selection.restart_after_update_guidance(),
        "Stop the old daemon to use the new version.\nStopping exits pane processes.\nRun `MAESTRO_SOCKET_PATH=custom.sock maestro daemon stop`, then restart Maestro with the same socket override."
    );
}
#[test]
#[cfg(unix)]
fn explicit_daemon_ignores_inherited_socket_override() {
    let selection = select(
        &["maestro", "attach", "--daemon=work"],
        SelectionEnvironment {
            api_socket: Some("api.sock"),
            client_socket: Some("client.sock"),
            ..Default::default()
        },
    );
    assert_eq!(selection.api_socket_path, "agent/daemons/work/daemon.sock");
    assert_eq!(
        selection.client_socket_path.to_str(),
        Some("agent/daemons/work/daemon-client.sock")
    );
}
#[test]
#[cfg(unix)]
fn socket_override_wins_for_implicit_selection() {
    let selection = select(
        &["maestro"],
        SelectionEnvironment {
            daemon_name: Some("work"),
            api_socket: Some("api.sock"),
            client_socket: Some("client.sock"),
        },
    );
    assert_eq!(selection.api_socket_path, "api.sock");
    assert_eq!(
        selection.client_socket_path.to_str(),
        Some("api-client.sock")
    );
}
#[test]
#[cfg(unix)]
fn socket_override_avoids_invalid_inherited_name() {
    for env in [
        SelectionEnvironment {
            daemon_name: Some("bad/name"),
            api_socket: Some("api.sock"),
            client_socket: None,
        },
        SelectionEnvironment {
            daemon_name: Some("bad/name"),
            api_socket: None,
            client_socket: Some("client.sock"),
        },
    ] {
        let selection = select(&["maestro"], env);
        assert_eq!(selection.paths.name(), None);
    }
}
#[test]
fn daemon_names_reject_invalid_components() {
    for invalid in [
        "",
        ".",
        "..",
        "../escape",
        "a/b",
        "a\\b",
        "é",
        &"a".repeat(65),
    ] {
        assert!(validate_name(invalid).is_err(), "{invalid}");
    }
    for valid in ["a", "A.b_0-z", &"a".repeat(64)] {
        assert!(validate_name(valid).is_ok(), "{valid}");
    }
}
#[test]
fn default_target_normalizes_to_root_daemon() {
    assert_eq!(parse_target_name("default").unwrap(), None);
    assert_eq!(parse_target_name("work").unwrap().as_deref(), Some("work"));
}
#[test]
#[cfg(unix)]
fn default_daemon_cannot_be_deleted() {
    assert_eq!(
        delete_session("unused", "default").unwrap_err(),
        "deleting the default daemon is not supported"
    );
}
#[test]
#[cfg(unix)]
fn daemon_listing_skips_reserved_default_directory() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path();
    for name in ["z", "a", "default", "bad name"] {
        std::fs::create_dir_all(root.join("daemons").join(name)).unwrap();
    }
    std::fs::write(root.join("daemons/file"), "not a directory").unwrap();
    let entries = list_sessions(root.to_str().unwrap()).unwrap();
    assert_eq!(
        entries
            .iter()
            .map(|info| info.name.as_str())
            .collect::<Vec<_>>(),
        ["default", "a", "z"]
    );
    assert!(entries[0].default);
    assert!(!entries[1].default);
    assert!(!entries[1].running);
    assert_eq!(
        entries[1].socket_path,
        format!("{}/daemons/a/daemon.sock", root.display())
    );
    assert_eq!(
        entries[1].session_dir,
        format!("{}/daemons/a", root.display())
    );
}

#[test]
#[cfg(unix)]
fn selection_keeps_chat_arguments_and_reports_target_errors() {
    let selected = select(
        &["maestro", "--session", "chat", "--daemon=unused"],
        SelectionEnvironment::default(),
    );
    assert!(!selected.explicit);
    assert_eq!(selected.paths.name(), None);
    let args = ["maestro", "attach", "--daemon"].map(str::to_owned);
    assert_eq!(
        Selection::from_args("agent", &args, None, SelectionEnvironment::default()).unwrap_err(),
        "missing value for --daemon"
    );
    let args = ["maestro", "attach", "--daemon="].map(str::to_owned);
    assert_eq!(
        Selection::from_args("agent", &args, None, SelectionEnvironment::default()).unwrap_err(),
        "daemon name cannot be empty"
    );
    assert!(
        Selection::from_args(
            "agent",
            &[],
            None,
            SelectionEnvironment {
                daemon_name: Some("../escape"),
                ..Default::default()
            }
        )
        .is_err()
    );
}
