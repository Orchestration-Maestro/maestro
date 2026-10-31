//! Static descriptions shared by chat help and command metadata.

/// Chat option descriptions; continuation indentation belongs to the text adapter.
pub(super) const OPTIONS: &[(&str, &str)] = &[
    (
        "provider",
        "Provider name (default: configured or available provider)",
    ),
    (
        "model",
        "Model pattern or ID (supports \"provider/id\" and optional \":<thinking>\")",
    ),
    ("api-key", "API key (defaults to env vars)"),
    (
        "system-prompt",
        "System prompt (default: coding assistant prompt)",
    ),
    (
        "append-system-prompt",
        "Append text or file contents to the system prompt (can be used multiple times)",
    ),
    ("mode", "Output mode: text (default), json, or rpc"),
    ("print", "Non-interactive mode: process prompt and exit"),
    ("continue", "Continue previous session"),
    ("resume", "Select a session to resume"),
    ("session", "Use specific session file or partial UUID"),
    (
        "fork",
        "Fork specific session file or partial UUID into a new session",
    ),
    ("session-dir", "Directory for session storage and lookup"),
    ("no-session", "Don't save session (ephemeral)"),
    (
        "models",
        "Comma-separated model patterns for Ctrl+P cycling\n                                 Supports globs (anthropic/*, *sonnet*) and fuzzy matching",
    ),
    (
        "no-tools",
        "Disable all tools by default (built-in and extension)",
    ),
    (
        "no-builtin-tools",
        "Disable built-in tools by default but keep extension/custom tools enabled",
    ),
    (
        "tools",
        "Comma-separated allowlist of tool names to enable\n                                 Applies to built-in, extension, and custom tools",
    ),
    (
        "thinking",
        "Set thinking level: off, minimal, low, medium, high, xhigh",
    ),
    (
        "extension",
        "Load an extension file (can be used multiple times)",
    ),
    (
        "no-extensions",
        "Disable extension discovery (explicit -e paths still work)",
    ),
    (
        "skill",
        "Load a skill file or directory (can be used multiple times)",
    ),
    ("no-skills", "Disable skills discovery and loading"),
    (
        "prompt-template",
        "Load a prompt template file or directory (can be used multiple times)",
    ),
    (
        "no-prompt-templates",
        "Disable prompt template discovery and loading",
    ),
    (
        "theme",
        "Load a theme file or directory (can be used multiple times)",
    ),
    ("no-themes", "Disable theme discovery and loading"),
    (
        "no-context-files",
        "Disable AGENTS.md and CLAUDE.md discovery and loading",
    ),
    ("export", "Export session file to HTML and exit"),
    (
        "list-models",
        "List available models (with optional fuzzy search)",
    ),
    (
        "verbose",
        "Force verbose startup (overrides quietStartup setting)",
    ),
    (
        "offline",
        "Disable startup network operations (same as %APP_UPPER%_OFFLINE=1)",
    ),
    ("help", "Show this help"),
    ("version", "Show version number"),
];
