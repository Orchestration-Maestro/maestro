//! Chat options described without replacing their consuming parser.

use super::{flag, option};
use clap::{Arg, ArgAction, Command, ValueHint};

/// Canonical flag names described by the chat help owner.
const FLAGS: &[&str] = &[
    "help",
    "version",
    "continue",
    "resume",
    "no-session",
    "no-tools",
    "no-builtin-tools",
    "no-extensions",
    "no-skills",
    "no-prompt-templates",
    "no-themes",
    "no-context-files",
    "verbose",
    "offline",
];

/// Single-valued chat options and their displayed argument labels.
const VALUES: &[(&str, &str)] = &[
    ("provider", "NAME"),
    ("model", "PATTERN"),
    ("api-key", "KEY"),
    ("system-prompt", "TEXT"),
    ("session", "PATH|ID"),
    ("fork", "PATH|ID"),
    ("session-dir", "DIR"),
    ("export", "FILE"),
    ("models", "PATTERNS"),
    ("tools", "TOOLS"),
];

/// Describe the chat flags, their values and prompt/file operands.
pub(super) fn configure(command: Command) -> Command {
    crate::input::args::describe_options(
        command
            .disable_help_flag(true)
            .disable_version_flag(true)
            .args(FLAGS.iter().map(|&name| short(flag(name))))
            .args(
                VALUES
                    .iter()
                    .map(|&(name, value)| short(option(name, value))),
            )
            .args(
                [
                    "append-system-prompt",
                    "extension",
                    "skill",
                    "prompt-template",
                    "theme",
                ]
                .map(|name| short(option(name, "VALUE").action(ArgAction::Append))),
            )
            .arg(option("mode", "MODE").value_parser(["text", "json", "rpc"]))
            .arg(
                option("thinking", "LEVEL")
                    .value_parser(["off", "minimal", "low", "medium", "high", "xhigh"]),
            )
            .arg(option("print", "PROMPT").short('p').num_args(0..=1))
            .arg(option("list-models", "SEARCH").num_args(0..=1))
            .arg(
                Arg::new("messages")
                    .value_name("@FILE|MESSAGE")
                    .num_args(0..)
                    .value_hint(ValueHint::Other),
            ),
    )
}

/// Retain single-letter shorts and describe the parser's multi-letter spellings.
fn short(arg: Arg) -> Arg {
    match arg.get_long() {
        Some("help") => arg.short('h'),
        Some("version") => arg.short('v'),
        Some("continue") => arg.short('c'),
        Some("resume") => arg.short('r'),
        Some("tools") => arg.short('t'),
        Some("no-tools") => arg.help("(-nt)"),
        Some("no-builtin-tools") => arg.help("(-nbt)"),
        Some("no-extensions") => arg.help("(-ne)"),
        Some("no-skills") => arg.help("(-ns)"),
        Some("no-prompt-templates") => arg.help("(-np)"),
        Some("no-context-files") => arg.help("(-nc)"),
        Some("extension") => arg.short('e'),
        _ => arg,
    }
}
