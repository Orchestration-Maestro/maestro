//! Ordinary slash commands and shared command provenance.

pub use maestro_resources::{SlashCommandInfo, SlashCommandSource};
use std::sync::LazyLock;

/// A built-in command's presentation metadata.
#[derive(Debug, Clone)]
pub struct BuiltinSlashCommand {
    /// Literal command name without the slash.
    pub name: String,
    /// Description shown in command selection.
    pub description: String,
    /// Placeholder shown beside the command in completion.
    pub argument_hint: Option<String>,
}

/// Ordinary built-in commands in presentation order.
pub static BUILTIN_SLASH_COMMANDS: LazyLock<Vec<BuiltinSlashCommand>> = LazyLock::new(|| {
    [
        ("settings", "Open settings menu"),
        ("model", "Select model (opens selector UI)"),
        ("thinking", "Set thinking level"),
        ("scoped-models", "Enable/disable models for Ctrl+P cycling"),
        (
            "export",
            "Export session (HTML default, or specify path: .html/.jsonl)",
        ),
        ("import", "Import and resume a session from a JSONL file"),
        ("share", "Share session as a secret GitHub gist"),
        ("copy", "Copy last agent message to clipboard"),
        ("name", "Set session display name"),
        ("session", "Show session info and stats"),
        ("changelog", "Show changelog entries"),
        ("hotkeys", "Show all keyboard shortcuts"),
        ("fork", "Create a new fork from a previous user message"),
        (
            "clone",
            "Duplicate the current session at the current position",
        ),
        ("tree", "Navigate session tree (switch branches)"),
        ("login", "Configure provider authentication"),
        ("logout", "Remove provider authentication"),
        ("new", "Start a new session"),
        ("compact", "Manually compact the session context"),
        ("resume", "Resume a different session"),
        (
            "reload",
            "Reload keybindings, extensions, skills, prompts, and themes",
        ),
    ]
    .into_iter()
    .map(|(name, description)| BuiltinSlashCommand {
        name: name.into(),
        description: description.into(),
        argument_hint: (name == "thinking").then(|| "<level>".into()),
    })
    .chain(std::iter::once(BuiltinSlashCommand {
        name: "quit".into(),
        description: format!("Quit {}", crate::configuration::config::APP_NAME),
        argument_hint: None,
    }))
    .collect()
});
