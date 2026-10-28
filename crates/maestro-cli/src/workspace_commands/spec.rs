//! Effect-free command metadata shared by help and shell completion.

use clap::{Arg, ArgAction, Command};

/// Describe the complete command-line surface without consuming chat input.
#[must_use]
pub fn command() -> Command {
    Command::new("maestro")
        .about("Coding conversations and terminal workspaces")
        .disable_help_subcommand(true)
        .subcommand(daemon())
        .subcommands(
            [
                ("attach", "Attach to a workspace daemon"),
                ("status", "Show client and daemon status"),
                ("api", "Inspect the workspace API"),
                ("completion", "Generate shell completion scripts"),
                ("settings", "Inspect workspace settings"),
                ("workspace", "Manage workspaces"),
                ("worktree", "Manage Git worktree workspaces"),
                ("tab", "Manage tabs"),
                ("pane", "Control terminal panes"),
                ("agent", "Control recognized pane agents"),
                ("terminal", "Attach to or observe terminal streams"),
                ("notification", "Show workspace notifications"),
                ("integration", "Manage agent integrations"),
                ("action", "Inspect and run workspace actions"),
            ]
            .map(|(name, about)| Command::new(name).about(about)),
        )
}

/// Describe the unified daemon lifecycle grammar.
fn daemon() -> Command {
    Command::new("daemon")
        .about("Run or control a workspace daemon")
        .arg(option("daemon", "NAME").global(true))
        .subcommand(
            Command::new("stop")
                .about("Stop the selected daemon")
                .arg(flag("json")),
        )
        .subcommand(
            Command::new("reload")
                .about("Reload settings or local detection manifests")
                .arg(flag("manifests")),
        )
        .subcommand(
            Command::new("manifests")
                .about("Show detection manifest status")
                .arg(flag("json")),
        )
        .subcommand(
            Command::new("update-manifests")
                .about("Fetch and reload detection manifests")
                .arg(flag("json")),
        )
        .subcommand(
            Command::new("live-handoff")
                .about("Transfer live panes to a new local daemon")
                .arg(option("import-exe", "PATH"))
                .arg(option("expected-protocol", "NUMBER"))
                .arg(option("expected-version", "VERSION")),
        )
        .subcommand(
            Command::new("list")
                .about("List named daemons")
                .arg(flag("json")),
        )
        .subcommand(
            Command::new("delete")
                .about("Delete a stopped named daemon")
                .arg(flag("json")),
        )
        .subcommand(
            Command::new("attach")
                .about("Print guidance for attaching to a named daemon")
                .arg(Arg::new("name").required(true)),
        )
}

/// Describe a flag that does not consume a value.
pub(super) fn flag(name: &'static str) -> Arg {
    Arg::new(name).long(name).action(ArgAction::SetTrue)
}

/// Describe an option with one value.
pub(super) fn option(name: &'static str, value: &'static str) -> Arg {
    Arg::new(name).long(name).value_name(value)
}
