//! Recognized agent command metadata.

use super::{flag, option, path, required, snapshot};
use clap::{Arg, ArgAction, ArgGroup, Command};

/// Describe agent inspection, input and lifecycle operations.
pub(super) fn command() -> Command {
    Command::new("agent").about("Control recognized pane agents")
        .subcommand(Command::new("list").about("List agents"))
        .subcommands([("get", "Show an agent"), ("focus", "Focus an agent")]
            .map(|(name, about)| Command::new(name).about(about).arg(required("target"))))
        .subcommand(Command::new("read").about("Read agent terminal output")
            .arg(required("target")).args(snapshot()).arg(flag("ansi")))
        .subcommand(Command::new("send-keys").about("Send key presses to an agent")
            .args([required("target"), required("key").num_args(1..)])
            .after_help("Use esc for Escape; escape is also accepted."))
        .subcommand(Command::new("wait").about("Wait for an observed agent state")
            .args([required("target"), until(), option("timeout", "MS")])
            .after_help("Without --until, matches idle, done or blocked. Without --timeout, waits indefinitely."))
        .subcommand(prompt()).subcommand(start()).subcommand(rename())
        .subcommand(Command::new("attach").about("Attach directly to an agent terminal")
            .args([required("target"), flag("takeover")]))
        .subcommand(Command::new("explain").about("Explain agent detection state")
            .args([Arg::new("target"), path("file"), option("agent", "LABEL"), flag("json"),
                option("format", "FORMAT").value_parser(["text", "json"]), flag("verbose").short('v')]))
}

/// Describe exact state matching without introducing additional wait variants.
fn until() -> Arg {
    option("until", "STATUS")
        .action(ArgAction::Append)
        .value_parser(["idle", "working", "blocked", "done", "unknown"])
}

/// Describe prompt submission and its optional observed-state wait.
fn prompt() -> Command {
    Command::new("prompt").about("Submit a prompt to an agent")
        .args([required("target"), required("text"), flag("wait"), until().requires("wait"),
            option("timeout", "MS").requires("wait")])
        .after_help("An already blocked agent rejects input. A submission from a non-working state must first change state within 5000ms or returns agent_prompt_stalled. A shorter --timeout returns timeout. The wait observes states, not turns: an already working turn can match. Defaults: idle, done or blocked; no timeout means an indefinite settled-state wait.")
}

/// Describe starting a supported agent in an existing shell pane.
fn start() -> Command {
    Command::new("start").about("Start a supported agent in an existing shell pane")
        .args([required("name"), option("kind", "KIND").required(true)
            .value_parser(maestro_app::daemon::Agent::ALL.map(maestro_app::daemon::agent_label)),
            option("pane", "ID").required(true), option("timeout", "MS"),
            Arg::new("agent_args").value_name("AGENT_ARG").num_args(0..).last(true)])
        .after_help("The pane must be at its interactive shell prompt. Success means the expected agent is detected and ready. Readiness timeout: 30000ms by default, 300000ms maximum.\n\nNext: maestro agent prompt <TARGET> <TEXT> --wait")
}

/// Describe exactly one name replacement or clear operation.
fn rename() -> Command {
    Command::new("rename")
        .about("Rename an agent")
        .args([required("target"), Arg::new("name"), flag("clear")])
        .group(
            ArgGroup::new("rename")
                .args(["name", "clear"])
                .required(true),
        )
}
