/// Classification and permitted direct dependencies for a scoped crate.
pub(crate) struct Rule {
    /// Required workspace classification for this crate.
    pub(crate) class: &'static str,
    /// Names of permitted direct internal dependencies.
    pub(crate) dependencies: &'static [&'static str],
}
/// Scoped crate names paired with their permitted direct dependencies.
pub(crate) const POLICY: &[(&str, &[&str])] = &[
    ("maestro-path", &[]),
    ("maestro-cancellation", &[]),
    ("maestro-request", &[]),
    ("maestro-extensions-wasm", &["maestro-request"]),
    (
        "maestro-models",
        &["maestro-cancellation", "maestro-request"],
    ),
    ("maestro-resources", &["maestro-request"]),
    ("maestro-settings", &[]),
    ("maestro-storage", &[]),
    ("maestro-test-conventions", &[]),
    ("maestro-tooling", &[]),
    ("maestro-tui", &["maestro-cancellation"]),
    ("maestro-agent", &["maestro-models"]),
    ("maestro-credentials", &["maestro-models"]),
    (
        "maestro-packages",
        &["maestro-settings", "maestro-resources"],
    ),
    ("maestro-test-terminal", &["maestro-tui"]),
    ("maestro-theme", &["maestro-tui", "maestro-request"]),
    ("maestro-tui-crossterm", &["maestro-tui"]),
    (
        "maestro-catalog",
        &["maestro-models", "maestro-credentials"],
    ),
    (
        "maestro-session",
        &["maestro-models", "maestro-agent", "maestro-storage"],
    ),
    (
        "maestro-tools",
        &[
            "maestro-models",
            "maestro-agent",
            "maestro-tui",
            "maestro-theme",
        ],
    ),
    (
        "maestro-export",
        &[
            "maestro-session",
            "maestro-models",
            "maestro-tools",
            "maestro-theme",
            "maestro-tui",
        ],
    ),
    (
        "maestro-extensions",
        &[
            "maestro-models",
            "maestro-agent",
            "maestro-session",
            "maestro-catalog",
            "maestro-tools",
            "maestro-theme",
            "maestro-tui",
            "maestro-resources",
        ],
    ),
    (
        "maestro-app",
        &[
            "maestro-models",
            "maestro-agent",
            "maestro-credentials",
            "maestro-settings",
            "maestro-storage",
            "maestro-catalog",
            "maestro-session",
            "maestro-tools",
            "maestro-resources",
            "maestro-packages",
            "maestro-extensions",
            "maestro-export",
            "maestro-theme",
            "maestro-tui",
        ],
    ),
    ("maestro-extensions-wasmtime", &["maestro-extensions"]),
    (
        "maestro-chat",
        &[
            "maestro-app",
            "maestro-tui",
            "maestro-tui-crossterm",
            "maestro-theme",
        ],
    ),
    (
        "maestro-cli",
        &[
            "maestro-app",
            "maestro-tui",
            "maestro-tui-crossterm",
            "maestro-theme",
        ],
    ),
    ("maestro-rpc", &["maestro-app", "maestro-theme"]),
    ("maestro-web", &["maestro-app", "maestro-theme"]),
    (
        "maestro",
        &[
            "maestro-app",
            "maestro-cli",
            "maestro-rpc",
            "maestro-chat",
            "maestro-web",
            "maestro-extensions-wasmtime",
        ],
    ),
];

/// Foundation utility that native crates may use beyond their table row.
pub(crate) const UTILITY: &str = "maestro-path";

/// Report whether a crate may declare the foundation utility as a production dependency.
pub(crate) fn uses_utility(name: &str) -> bool {
    name != UTILITY
        && !matches!(
            name,
            "maestro-cancellation"
                | "maestro-request"
                | "maestro-extensions-wasm"
                | "maestro-extensions-wasmtime"
                | "maestro-test-terminal"
        )
}

/// Look up a scoped crate's classification and permitted dependencies.
pub(crate) fn rule(name: &str) -> Option<Rule> {
    let dependencies = POLICY.iter().find(|row| row.0 == name)?.1;
    Some(Rule {
        class: if matches!(
            name,
            "maestro" | "maestro-test-conventions" | "maestro-test-terminal" | "maestro-tooling"
        ) {
            "dedicated"
        } else {
            "core"
        },
        dependencies,
    })
}

/// Identify crates that require their full direct dependency set.
pub(crate) fn exact(name: &str) -> bool {
    matches!(
        name,
        "maestro-cli"
            | "maestro-chat"
            | "maestro-rpc"
            | "maestro-web"
            | "maestro-tui-crossterm"
            | "maestro-test-terminal"
    )
}
