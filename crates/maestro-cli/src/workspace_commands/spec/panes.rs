//! Pane topology, terminal input and reporting metadata.

use super::{env, flag, option, path, required, snapshot};
use clap::{Arg, ArgAction, ArgGroup, Command};

/// Describe pane topology, input and reporting commands.
pub(super) fn command() -> Command {
    Command::new("pane")
        .about("Control terminal panes")
        .subcommand(
            Command::new("list")
                .about("List panes")
                .arg(option("workspace", "WORKSPACE_ID")),
        )
        .subcommands(
            [
                ("current", "Show the caller's pane"),
                ("layout", "Show pane layout"),
                ("process-info", "Show pane process information"),
                ("edges", "Show pane edges"),
            ]
            .map(|(name, about)| Command::new(name).about(about).args(current())),
        )
        .subcommands(
            [
                ("neighbor", "Find a neighboring pane"),
                ("focus", "Focus a neighboring pane"),
                ("resize", "Resize a pane split"),
            ]
            .map(|(name, about)| {
                Command::new(name)
                    .about(about)
                    .args(current())
                    .arg(direction().required(true))
                    .args((name == "resize").then(|| option("amount", "FLOAT")))
            }),
        )
        .subcommands(
            [("get", "Show a pane"), ("close", "Close a pane")]
                .map(|(name, about)| Command::new(name).about(about).arg(required("pane_id"))),
        )
        .subcommands(display_commands())
        .subcommand(split())
        .subcommand(move_pane())
        .subcommand(wait_output())
        .subcommand(
            Command::new("swap")
                .about("Swap panes")
                .args(current())
                .args([
                    direction(),
                    option("source-pane", "ID"),
                    option("target-pane", "ID"),
                ]),
        )
        .subcommands(input_commands())
        .subcommands(report_commands())
}

/// Describe pane display, naming and input-routing operations.
fn display_commands() -> [Command; 4] {
    [
        Command::new("zoom")
            .about("Toggle or set pane zoom")
            .arg(Arg::new("pane_id"))
            .args(current())
            .args([flag("toggle"), flag("on"), flag("off")]),
        Command::new("read")
            .about("Read pane terminal output")
            .arg(required("pane_id"))
            .args(snapshot())
            .args([flag("ansi"), flag("raw")]),
        Command::new("rename").about("Rename a pane").args([
            required("pane_id"),
            Arg::new("label").num_args(1..),
            flag("clear"),
        ]),
        Command::new("input")
            .about("Set pane input routing")
            .arg(Arg::new("pane_id"))
            .args(current())
            .arg(
                option("right-click", "TARGET")
                    .required(true)
                    .value_parser(["maestro", "pane"]),
            ),
    ]
}

/// Describe explicit caller-pane selectors.
fn current() -> [Arg; 2] {
    [option("pane", "ID"), flag("current")]
}

/// Describe the four spatial directions.
fn direction() -> Arg {
    option("direction", "DIRECTION").value_parser(["left", "right", "up", "down"])
}

/// Describe a split that preserves caller-provided environment and focus policy.
fn split() -> Command {
    Command::new("split")
        .about("Split a pane")
        .arg(Arg::new("pane_id"))
        .args(current())
        .args([
            option("direction", "DIRECTION").value_parser(["right", "down"]),
            option("ratio", "FLOAT"),
            path("cwd"),
            env(),
            option("right-click", "TARGET").value_parser(["maestro", "pane"]),
            flag("focus"),
            flag("no-focus"),
        ])
}

/// Describe moving a pane without predicting its new public identifier.
fn move_pane() -> Command {
    Command::new("move").about("Move a pane").args([
        required("pane_id"),
        option("tab", "TAB_ID"),
        option("split", "DIRECTION").value_parser(["right", "down"]),
        option("target-pane", "ID"),
        option("ratio", "FLOAT"),
        flag("new-tab"),
        option("workspace", "ID"),
        flag("new-workspace"),
        option("label", "TEXT"),
        option("tab-label", "TEXT"),
        flag("focus"),
        flag("no-focus"),
    ])
}

/// Describe matching a selected terminal snapshot, including existing output.
fn wait_output() -> Command {
    Command::new("wait-output").about("Wait for matching pane output")
        .args([required("pane_id"), option("match", "TEXT"), option("regex", "PATTERN"),
            option("source", "SOURCE").value_parser(["visible", "recent", "recent-unwrapped"]),
            option("lines", "N"), option("timeout", "MS"), flag("raw")])
        .group(ArgGroup::new("matcher").args(["match", "regex"]).required(true))
        .after_help("Searches the current snapshot immediately, then polls. Without --timeout, waits indefinitely.")
}

/// Describe raw terminal input and execution commands.
fn input_commands() -> [Command; 3] {
    [
        Command::new("send-text")
            .about("Send literal text to a pane")
            .args([required("pane_id"), required("text")])
            .after_help(
                "Next: maestro pane run <PANE_ID> <COMMAND> sends text and Enter in one call.",
            ),
        Command::new("send-keys")
            .about("Send key presses to a pane")
            .args([required("pane_id"), required("key").num_args(1..)])
            .after_help("Use esc for Escape; escape is also accepted."),
        Command::new("run")
            .about("Run a command in a pane")
            .args([required("pane_id"), required("command").num_args(1..)]),
    ]
}

/// Describe lifecycle authority and display metadata reports.
fn report_commands() -> [Command; 4] {
    [
        Command::new("report-agent")
            .about("Report pane agent lifecycle state")
            .args(authority())
            .args(session_identity())
            .args([
                option("state", "STATUS")
                    .required(true)
                    .value_parser(["idle", "working", "blocked", "unknown"]),
                option("message", "TEXT"),
            ]),
        Command::new("report-agent-session")
            .about("Report pane agent session identity")
            .args(authority())
            .args(session_identity())
            .arg(option("session-start-source", "SOURCE")),
        Command::new("release-agent")
            .about("Release pane agent lifecycle authority")
            .args(authority()),
        Command::new("report-metadata")
            .about("Report display-only pane metadata")
            .args([
                required("pane_id"),
                option("source", "ID").required(true),
                option("agent", "LABEL"),
                option("applies-to-source", "ID"),
                option("title", "TEXT"),
                flag("clear-title"),
                option("display-agent", "TEXT"),
                flag("clear-display-agent"),
                option("state-label", "STATUS=TEXT"),
                flag("clear-state-labels"),
                option("token", "NAME=VALUE").action(ArgAction::Append),
                option("clear-token", "NAME").action(ArgAction::Append),
                option("seq", "N"),
                option("ttl-ms", "N"),
            ]),
    ]
}

/// Describe a pane authority identity and its optional sequence number.
fn authority() -> [Arg; 4] {
    [
        required("pane_id"),
        option("source", "ID").required(true),
        option("agent", "LABEL").required(true),
        option("seq", "N"),
    ]
}

/// Describe an optional native agent session reference.
fn session_identity() -> [Arg; 2] {
    [option("agent-session-id", "ID"), path("agent-session-path")]
}
