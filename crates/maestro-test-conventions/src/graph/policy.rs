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
    ("maestro-watch", &[]),
    ("maestro-lock", &[]),
    ("maestro-process", &[]),
    ("maestro-text", &[]),
    ("maestro-json", &[]),
    ("maestro-entry", &["maestro-request"]),
    ("maestro-tool", &["maestro-request"]),
    ("maestro-brand", &["maestro-text", "maestro-json"]),
    ("maestro-pattern", &["maestro-text"]),
    (
        "maestro-extensions-wasm",
        &[
            "maestro-request",
            "maestro-entry",
            "maestro-tool",
            "maestro-json",
        ],
    ),
    (
        "maestro-models",
        &[
            "maestro-cancellation",
            "maestro-request",
            "maestro-text",
            "maestro-json",
            "maestro-brand",
        ],
    ),
    (
        "maestro-resources",
        &["maestro-request", "maestro-pattern", "maestro-text"],
    ),
    ("maestro-settings", &["maestro-lock", "maestro-json"]),
    ("maestro-test-conventions", &[]),
    ("maestro-tooling", &[]),
    (
        "maestro-tui",
        &["maestro-cancellation", "maestro-text", "maestro-process"],
    ),
    (
        "maestro-agent",
        &["maestro-models", "maestro-text", "maestro-tool"],
    ),
    (
        "maestro-credentials",
        &[
            "maestro-models",
            "maestro-lock",
            "maestro-text",
            "maestro-process",
        ],
    ),
    (
        "maestro-packages",
        &[
            "maestro-settings",
            "maestro-resources",
            "maestro-pattern",
            "maestro-text",
            "maestro-json",
            "maestro-process",
        ],
    ),
    ("maestro-test-terminal", &["maestro-tui"]),
    (
        "maestro-theme",
        &[
            "maestro-tui",
            "maestro-request",
            "maestro-watch",
            "maestro-text",
            "maestro-json",
            "maestro-brand",
        ],
    ),
    ("maestro-tui-crossterm", &["maestro-tui", "maestro-text"]),
    (
        "maestro-catalog",
        &[
            "maestro-models",
            "maestro-credentials",
            "maestro-pattern",
            "maestro-request",
            "maestro-text",
            "maestro-json",
        ],
    ),
    (
        "maestro-session",
        &[
            "maestro-models",
            "maestro-agent",
            "maestro-request",
            "maestro-entry",
            "maestro-text",
            "maestro-json",
        ],
    ),
    (
        "maestro-tools",
        &[
            "maestro-models",
            "maestro-agent",
            "maestro-tui",
            "maestro-theme",
            "maestro-tool",
            "maestro-text",
            "maestro-process",
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
            "maestro-json",
            "maestro-text",
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
            "maestro-catalog",
            "maestro-session",
            "maestro-tools",
            "maestro-resources",
            "maestro-packages",
            "maestro-extensions",
            "maestro-export",
            "maestro-theme",
            "maestro-tui",
            "maestro-watch",
            "maestro-text",
            "maestro-json",
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
            "maestro-watch"
                | "maestro-lock"
                | "maestro-process"
                | "maestro-pattern"
                | "maestro-cancellation"
                | "maestro-request"
                | "maestro-extensions-wasm"
                | "maestro-extensions-wasmtime"
                | "maestro-test-terminal"
                | "maestro-text"
                | "maestro-json"
                | "maestro-entry"
                | "maestro-tool"
                | "maestro-brand"
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

/// Identify crates that require direct dependencies.
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

/// List the direct dependencies a crate with an exact boundary must declare.
pub(crate) fn required(name: &str) -> &'static [&'static str] {
    if exact(name) {
        rule(name).map_or(&[], |rule| rule.dependencies)
    } else {
        &[]
    }
}
