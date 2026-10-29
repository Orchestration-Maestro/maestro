//! Local discovery and miscellaneous workspace command metadata.

use super::{env, flag, option, path, required};
use crate::packages::package_manager_cli::{CONFIG_DESCRIPTION, PackageCommand, description};
use clap::{Arg, Command};

/// Describe attaching without changing bare-chat startup.
pub(super) fn attach() -> Command {
    Command::new("attach")
        .about("Attach to a workspace daemon")
        .args([
            option("daemon", "NAME"),
            flag("in-process"),
            option("remote", "TARGET"),
            option("remote-keybindings", "MODE").value_parser(["local", "server"]),
            flag("handoff"),
        ])
}

/// Describe effect-free discovery and settings operations.
pub(super) fn discovery() -> [Command; 4] {
    [
        Command::new("status")
            .about("Show client and daemon versions, protocol and restart requirements")
            .args([option("daemon", "NAME"), flag("json")]),
        Command::new("completion")
            .about("Generate shell completion scripts")
            .arg(required("shell").value_parser(crate::completion::SUPPORTED_SHELLS)),
        Command::new("settings")
            .about("Inspect workspace settings")
            .subcommands(
                [
                    ("check", "Validate settings.json and print diagnostics"),
                    (
                        "defaults",
                        "Print annotated JSONC defaults; settings.json remains strict JSON",
                    ),
                    (
                        "reset-keys",
                        "Back up keybindings.json and remove only multiplexer overrides",
                    ),
                ]
                .map(|(name, about)| Command::new(name).about(about)),
            ),
        Command::new("api")
            .about("Inspect workspace API metadata and live state")
            .subcommand(Command::new("snapshot").about("Print the live workspace snapshot"))
            .subcommand(
                Command::new("schema")
                    .about("Print or write the API schema")
                    .args([flag("json"), path("output")]),
            ),
    ]
}

/// Describe terminal stream attachment and title operations.
pub(super) fn terminal() -> Command {
    Command::new("terminal")
        .about("Attach to or observe terminal streams")
        .subcommand(
            Command::new("attach")
                .about("Attach directly to a terminal stream")
                .args([required("terminal_id"), flag("takeover")]),
        )
        .subcommand(
            Command::new("session")
                .about("Work with terminal sessions")
                .subcommands(
                    [
                        ("control", "Control a terminal stream"),
                        ("observe", "Observe a terminal stream"),
                    ]
                    .map(|(name, about)| {
                        Command::new(name)
                            .about(about)
                            .args([required("target"), option("cols", "N"), option("rows", "N")])
                            .args((name == "control").then(|| flag("takeover")))
                    }),
                ),
        )
        .subcommand(
            Command::new("title")
                .about("Manage the outer terminal title")
                .subcommand(
                    Command::new("set")
                        .about("Set the outer terminal title")
                        .arg(required("title")),
                )
                .subcommand(Command::new("clear").about("Clear the outer terminal title")),
        )
}

/// Describe notification presentation and supported integration targets.
pub(super) fn notifications_and_integrations() -> [Command; 2] {
    let values =
        serde_json::json!(maestro_app::rpc_types::workspace_schema::IntegrationTarget::ALL);
    let targets: Vec<_> = values
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(str::to_owned)
        .collect();
    [
        Command::new("notification")
            .about("Show workspace notifications")
            .subcommand(Command::new("show").about("Show a notification").args([
                required("title"),
                option("body", "TEXT"),
                option("position", "POSITION").value_parser([
                    "top-left",
                    "top-right",
                    "bottom-left",
                    "bottom-right",
                ]),
                option("sound", "SOUND").value_parser(["none", "done", "request"]),
            ])),
        Command::new("integration")
            .about("Manage supported agent integrations")
            .subcommands(
                [
                    ("install", "Install an integration"),
                    ("uninstall", "Uninstall an integration"),
                ]
                .map(|(name, about)| {
                    Command::new(name)
                        .about(about)
                        .arg(required("target").value_parser(targets.clone()))
                }),
            )
            .subcommand(
                Command::new("status")
                    .about("Show integration status")
                    .arg(flag("outdated-only")),
            ),
    ]
}

/// Describe action operations without a second package manager.
pub(super) fn action() -> Command {
    Command::new("action")
        .about("Inspect and run workspace actions")
        .subcommands(action_manifests())
        .subcommand(
            Command::new("list")
                .about("List workspace actions")
                .arg(option("action-manifest", "ID")),
        )
        .subcommand(
            Command::new("invoke")
                .about("Invoke a workspace action")
                .args([required("action_id"), option("action-manifest", "ID")]),
        )
        .subcommand(
            Command::new("config-dir")
                .about("Print an action package configuration directory")
                .arg(required("manifest_id")),
        )
        .subcommand(
            Command::new("log")
                .about("Inspect action command logs")
                .subcommand(
                    Command::new("list")
                        .about("List action command logs")
                        .args([option("action-manifest", "ID"), option("limit", "N")]),
                ),
        )
        .subcommand(
            Command::new("pane")
                .about("Manage action-owned panes")
                .subcommand(action_pane_open())
                .subcommands(
                    [
                        ("focus", "Focus an action pane"),
                        ("close", "Close an action pane"),
                    ]
                    .map(|(name, about)| Command::new(name).about(about).arg(required("pane_id"))),
                ),
        )
}

/// Describe an action pane's placement and launch context.
fn action_pane_open() -> Command {
    Command::new("open").about("Open an action pane").args([
        option("action-manifest", "ID"),
        option("entrypoint", "ID"),
        option("placement", "PLACEMENT").value_parser(["overlay", "split", "tab", "zoomed"]),
        option("workspace", "ID"),
        option("target-pane", "PANE"),
        option("direction", "DIRECTION").value_parser(["right", "down"]),
        path("cwd"),
        env(),
        flag("focus"),
        flag("no-focus"),
    ])
}

/// Describe existing package commands without running package discovery.
pub(super) fn packages() -> [Command; 5] {
    [
        Command::new("install")
            .about(description(PackageCommand::Install))
            .args([required("source"), flag("local").short('l')]),
        Command::new("remove")
            .visible_alias("uninstall")
            .about(description(PackageCommand::Remove))
            .args([required("source"), flag("local").short('l')]),
        Command::new("update")
            .about(description(PackageCommand::Update))
            .args([
                Arg::new("source"),
                flag("self"),
                flag("extensions"),
                option("extension", "SOURCE"),
                flag("force"),
            ]),
        Command::new("list").about(description(PackageCommand::List)),
        Command::new("config").about(CONFIG_DESCRIPTION),
    ]
}

/// Describe action-manifest admission and enabled-state discovery.
fn action_manifests() -> [Command; 5] {
    [
        Command::new("link")
            .about("Link a local action manifest")
            .args([
                required("path").value_hint(clap::ValueHint::AnyPath),
                flag("disabled").conflicts_with("enabled"),
                flag("enabled"),
            ]),
        Command::new("remove")
            .about("Remove an action manifest")
            .arg(required("manifest_id")),
        Command::new("enable")
            .about("Enable an action manifest")
            .arg(required("manifest_id")),
        Command::new("disable")
            .about("Disable an action manifest")
            .arg(required("manifest_id")),
        Command::new("manifests")
            .about("List admitted action manifests")
            .args([option("action-manifest", "ID"), flag("json")]),
    ]
}
