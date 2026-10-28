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

const VALID_COMMANDS: &[&[&str]] = &[
    &[
        "workspace",
        "report-metadata",
        "w1",
        "--source",
        "status",
        "--token",
        "branch=main",
    ],
    &[
        "tab",
        "create",
        "--workspace",
        "w1",
        "--env",
        "A=1",
        "--env",
        "B=2",
        "--no-focus",
    ],
    &[
        "pane",
        "split",
        "--current",
        "--direction",
        "right",
        "--cwd",
        "/work",
        "--no-focus",
    ],
    &[
        "pane",
        "wait-output",
        "w1:p1",
        "--regex",
        "ready",
        "--timeout",
        "500",
    ],
    &[
        "agent", "prompt", "reviewer", "check", "--wait", "--until", "blocked",
    ],
    &["agent", "rename", "reviewer", "--clear"],
    &[
        "agent",
        "start",
        "reviewer",
        "--kind",
        "pi",
        "--pane",
        "w1:p1",
        "--",
        "--offline",
    ],
    &["worktree", "open", "--branch", "main"],
    &["terminal", "session", "observe", "w1:p1", "--rows", "24"],
    &[
        "action",
        "pane",
        "open",
        "--package",
        "build",
        "--placement",
        "zoomed",
    ],
    &["notification", "show", "Ready", "--position", "top-right"],
    &["integration", "status", "--outdated-only"],
    &["api", "schema", "--output", "schema.json"],
    &["attach", "--daemon", "build", "--handoff"],
    &["status", "--json"],
    &["settings", "defaults"],
    &["install", "./actions", "-l"],
];

const INVALID_COMMANDS: &[&[&str]] = &[
    &["agent", "prompt", "reviewer", "check", "--until", "idle"],
    &["agent", "rename", "reviewer"],
    &["agent", "rename", "reviewer", "worker", "--clear"],
    &["pane", "wait-output", "w1:p1"],
    &[
        "pane",
        "wait-output",
        "w1:p1",
        "--match",
        "x",
        "--regex",
        "x",
    ],
    &["pane", "focus", "--current"],
    &["workspace", "report-metadata", "w1"],
];

#[test]
fn command_metadata_describes_workspace_targets_and_safety_constraints() {
    for args in VALID_COMMANDS {
        assert!(
            command()
                .try_get_matches_from(std::iter::once("maestro").chain(args.iter().copied()))
                .is_ok(),
            "{args:?}"
        );
    }
    for args in INVALID_COMMANDS {
        assert!(
            command()
                .try_get_matches_from(std::iter::once("maestro").chain(args.iter().copied()))
                .is_err(),
            "{args:?}"
        );
    }
    command().debug_assert();
}

#[test]
fn command_metadata_reuses_agent_and_integration_vocabularies() {
    let root = command();
    let start = root
        .find_subcommand("agent")
        .unwrap()
        .find_subcommand("start")
        .unwrap();
    let kind = start
        .get_arguments()
        .find(|arg| arg.get_long() == Some("kind"))
        .unwrap();
    let values: Vec<_> = kind
        .get_value_parser()
        .possible_values()
        .unwrap()
        .map(|value| value.get_name().to_owned())
        .collect();
    assert_eq!(
        values,
        maestro_app::daemon::Agent::ALL.map(maestro_app::daemon::agent_label)
    );
    let install = root
        .find_subcommand("integration")
        .unwrap()
        .find_subcommand("install")
        .unwrap();
    let target = install
        .get_arguments()
        .find(|arg| arg.get_id() == "target")
        .unwrap();
    let values: Vec<_> = target
        .get_value_parser()
        .possible_values()
        .unwrap()
        .map(|value| value.get_name().to_owned())
        .collect();
    let expected: Vec<_> = maestro_app::rpc_types::workspace_schema::IntegrationTarget::ALL
        .into_iter()
        .map(|target| {
            serde_json::to_value(target)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(values, expected);
}
