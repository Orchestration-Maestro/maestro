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
        ("scoped-models", "Turn models on or off for Ctrl+P cycling"),
        (
            "export",
            "Export session (HTML default, or specify path: .html/.jsonl)",
        ),
        ("import", "Load a session from a JSONL file and resume it"),
        ("share", "Publish the session as a secret gist on GitHub"),
        ("copy", "Put the last agent message on the clipboard"),
        ("name", "Set session display name"),
        ("session", "Show session info and stats"),
        ("changelog", "Show changelog entries"),
        ("hotkeys", "Show all keyboard shortcuts"),
        (
            "fork",
            "Open a new session that forks off an earlier user turn",
        ),
        ("clone", "Clone the current session at its present position"),
        ("tree", "Navigate session tree (switch branches)"),
        ("login", "Configure provider authentication"),
        ("logout", "Remove provider authentication"),
        ("new", "Start a new session"),
        ("compact", "Manually compact the session context"),
        ("resume", "Resume a different session"),
        (
            "reload",
            "Refresh themes, prompts, skills, extensions and keybindings",
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
