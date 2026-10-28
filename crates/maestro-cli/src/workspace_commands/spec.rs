//! Effect-free command metadata shared by help and shell completion.

use clap::{Arg, ArgAction, Command, ValueHint};

mod agents;
mod chat;
mod local;
mod panes;
mod topology;

/// Describe the complete command-line surface without consuming chat input.
#[must_use]
pub fn command() -> Command {
    chat::configure(
        Command::new("maestro")
            .about("Coding conversations and terminal workspaces")
            .disable_help_subcommand(true)
            .subcommand(daemon())
            .subcommand(local::attach())
            .subcommands(local::discovery())
            .subcommand(topology::collection("workspace", "workspace_id"))
            .subcommand(topology::collection("tab", "tab_id"))
            .subcommand(topology::worktree())
            .subcommand(panes::command())
            .subcommand(agents::command())
            .subcommand(local::terminal())
            .subcommands(local::notifications_and_integrations())
            .subcommand(local::action())
            .subcommands(local::packages()),
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

/// Describe a required positional operand.
fn required(name: &'static str) -> Arg {
    Arg::new(name)
        .value_name(name.to_ascii_uppercase())
        .required(true)
}

/// Describe a native authored path without observing it.
fn path(name: &'static str) -> Arg {
    option(name, "PATH").value_hint(ValueHint::AnyPath)
}

/// Describe repeated environment assignments for a launched process.
fn env() -> Arg {
    option("env", "KEY=VALUE").action(ArgAction::Append)
}

/// Describe a terminal snapshot selection and display format.
fn snapshot() -> [Arg; 3] {
    [
        option("source", "SOURCE").value_parser([
            "visible",
            "recent",
            "recent-unwrapped",
            "detection",
        ]),
        option("lines", "N"),
        option("format", "FORMAT").value_parser(["text", "ansi"]),
    ]
}
