//! Effect-free workspace command discovery.
#![cfg(test)]

use maestro_cli::workspace_commands::spec::command;

#[test]
fn workspace_metadata_unifies_daemon_verbs() {
    let mut root = command();
    root.build();
    root.clone().debug_assert();
    for noun in [
        "attach",
        "daemon",
        "status",
        "api",
        "completion",
        "settings",
        "workspace",
        "worktree",
        "tab",
        "pane",
        "agent",
        "terminal",
        "notification",
        "integration",
        "action",
    ] {
        assert!(root.find_subcommand(noun).is_some(), "missing {noun}");
    }
    for removed in ["server", "session", "channel"] {
        assert!(root.find_subcommand(removed).is_none());
    }
    let daemon = root.find_subcommand("daemon").unwrap();
    let verbs: Vec<_> = daemon
        .get_subcommands()
        .map(clap::Command::get_name)
        .collect();
    assert_eq!(
        verbs,
        [
            "stop",
            "reload",
            "manifests",
            "update-manifests",
            "live-handoff",
            "list",
            "delete",
            "attach"
        ]
    );
    let parsed = root
        .try_get_matches_from(["maestro", "daemon", "delete", "--daemon", "build", "--json"])
        .unwrap();
    let delete = parsed
        .subcommand_matches("daemon")
        .unwrap()
        .subcommand_matches("delete")
        .unwrap();
    assert_eq!(
        delete.get_one::<String>("daemon").map(String::as_str),
        Some("build")
    );
    assert!(delete.get_flag("json"));
}
