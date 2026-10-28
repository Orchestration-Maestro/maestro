//! Chat options described without replacing their consuming parser.

use super::{flag, option};
use clap::{Arg, ArgAction, Command, ValueHint};

/// Flag descriptions, including the chat parser's multi-letter short spellings.
const FLAGS: &[(&str, &str)] = &[
    ("help", "Show help"),
    ("version", "Show version"),
    ("continue", "Continue the previous session"),
    ("resume", "Select a session to resume"),
    ("no-session", "Do not save the session"),
    ("no-tools", "Disable all tools (-nt)"),
    ("no-builtin-tools", "Disable built-in tools (-nbt)"),
    ("no-extensions", "Disable extension discovery (-ne)"),
    ("no-skills", "Disable skill discovery and loading (-ns)"),
    (
        "no-prompt-templates",
        "Disable prompt template discovery (-np)",
    ),
    ("no-themes", "Disable theme discovery and loading"),
    ("no-context-files", "Disable context file discovery (-nc)"),
    ("verbose", "Show verbose startup"),
    ("offline", "Disable startup network operations"),
];

/// Single-valued chat options and their displayed argument labels.
const VALUES: &[(&str, &str, &str)] = &[
    ("provider", "NAME", "Provider name"),
    ("model", "PATTERN", "Model pattern or identifier"),
    ("api-key", "KEY", "API key"),
    ("system-prompt", "TEXT", "Replace the system prompt"),
    (
        "session",
        "PATH|ID",
        "Use a session file or partial identifier",
    ),
    ("fork", "PATH|ID", "Fork a session"),
    ("session-dir", "DIR", "Session storage directory"),
    ("export", "FILE", "Export a session to HTML"),
    (
        "models",
        "PATTERNS",
        "Comma-separated model patterns for cycling",
    ),
    ("tools", "TOOLS", "Comma-separated tool allowlist"),
];

/// Describe the chat flags, their values and prompt/file operands.
pub(super) fn configure(command: Command) -> Command {
    command
        .disable_help_flag(true)
        .disable_version_flag(true)
        .args(
            FLAGS
                .iter()
                .map(|&(name, help)| short(flag(name).help(help))),
        )
        .args(
            VALUES
                .iter()
                .map(|&(name, value, help)| short(option(name, value).help(help))),
        )
        .args(
            [
                "append-system-prompt",
                "extension",
                "skill",
                "prompt-template",
                "theme",
            ]
            .map(|name| {
                short(
                    option(name, "VALUE")
                        .action(ArgAction::Append)
                        .help("Load an explicit resource; may be repeated"),
                )
            }),
        )
        .arg(
            option("mode", "MODE")
                .value_parser(["text", "json", "rpc"])
                .help("Output mode"),
        )
        .arg(
            option("thinking", "LEVEL")
                .value_parser(["off", "minimal", "low", "medium", "high", "xhigh"])
                .help("Thinking level"),
        )
        .arg(
            option("print", "PROMPT")
                .short('p')
                .num_args(0..=1)
                .help("Process a prompt without interaction"),
        )
        .arg(
            option("list-models", "SEARCH")
                .num_args(0..=1)
                .help("List available models"),
        )
        .arg(
            Arg::new("messages")
                .value_name("@FILE|MESSAGE")
                .num_args(0..)
                .value_hint(ValueHint::Other),
        )
}

/// Retain the parser's single-letter short spellings.
fn short(arg: Arg) -> Arg {
    match arg.get_long() {
        Some("help") => arg.short('h'),
        Some("version") => arg.short('v'),
        Some("continue") => arg.short('c'),
        Some("resume") => arg.short('r'),
        Some("tools") => arg.short('t'),
        Some("extension") => arg.short('e'),
        _ => arg,
    }
}
