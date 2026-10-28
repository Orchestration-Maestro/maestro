//! Workspace, tab and worktree command metadata.

use super::{env, flag, option, path, required};
use clap::Command;

/// Describe the shared workspace and tab operations.
pub(super) fn collection(noun: &'static str, id: &'static str) -> Command {
    let mut command = Command::new(noun)
        .about(format!("Manage {noun}s"))
        .subcommand(Command::new("list").about(format!("List {noun}s")))
        .subcommand(
            Command::new("create")
                .about(format!("Create a {noun}"))
                .args([
                    path("cwd"),
                    option("label", "TEXT"),
                    env(),
                    flag("focus"),
                    flag("no-focus"),
                ]),
        )
        .subcommands(
            [("get", "Show"), ("focus", "Focus"), ("close", "Close")].map(|(name, verb)| {
                Command::new(name)
                    .about(format!("{verb} a {noun}"))
                    .arg(required(id))
            }),
        )
        .subcommand(
            Command::new("rename")
                .about(format!("Rename a {noun}"))
                .args([required(id), required("label").num_args(1..)]),
        );
    if noun == "tab" {
        command = command
            .mut_subcommand("list", |cmd| cmd.arg(option("workspace", "WORKSPACE_ID")))
            .mut_subcommand("create", |cmd| cmd.arg(option("workspace", "WORKSPACE_ID")));
    } else {
        command = command.subcommand(
            Command::new("report-metadata")
                .about("Report display-only workspace metadata")
                .args([
                    required(id),
                    option("source", "ID").required(true),
                    option("token", "NAME=VALUE").action(clap::ArgAction::Append),
                    option("clear-token", "NAME").action(clap::ArgAction::Append),
                    option("seq", "N"),
                    option("ttl-ms", "N"),
                ]),
        );
    }
    command
}

/// Describe Git worktree discovery and mutations.
pub(super) fn worktree() -> Command {
    Command::new("worktree")
        .about("Manage Git worktree workspaces")
        .subcommand(
            Command::new("list")
                .about("List worktree workspaces")
                .args([option("workspace", "ID"), path("cwd")]),
        )
        .subcommands(
            [
                ("create", "Create and open a Git worktree"),
                ("open", "Open an existing Git worktree"),
            ]
            .map(|(name, about)| {
                Command::new(name)
                    .about(about)
                    .args([
                        option("workspace", "ID"),
                        path("cwd"),
                        option("branch", "NAME"),
                        path("path"),
                        option("label", "TEXT"),
                        flag("focus"),
                        flag("no-focus"),
                    ])
                    .args((name == "create").then(|| option("base", "REF")))
            }),
        )
        .subcommand(
            Command::new("remove")
                .about("Remove a worktree checkout")
                .args([option("workspace", "ID"), flag("force")]),
        )
}
