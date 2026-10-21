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
    ("maestro-request", &["maestro-json"]),
    ("maestro-watch", &[]),
    ("maestro-lock", &[]),
    ("maestro-process", &[]),
    ("maestro-listing", &[]),
    ("maestro-protocol", &[]),
    ("maestro-ipc", &[]),
    ("maestro-terminal", &["maestro-text"]),
    (
        "maestro-workspace",
        &[
            "maestro-protocol",
            "maestro-json",
            "maestro-text",
            "maestro-lock",
        ],
    ),
    (
        "maestro-detection",
        &[
            "maestro-protocol",
            "maestro-text",
            "maestro-process",
            "maestro-lock",
        ],
    ),
    (
        "maestro-daemon",
        &[
            "maestro-protocol",
            "maestro-ipc",
            "maestro-process",
            "maestro-terminal",
            "maestro-workspace",
            "maestro-detection",
            "maestro-settings",
            "maestro-tui",
            "maestro-json",
            "maestro-text",
        ],
    ),
    // Common text operations include CSS string serialization for brand and application callers.
    ("maestro-text", &[]),
    ("maestro-json", &[]),
    ("maestro-entry", &["maestro-request"]),
    ("maestro-tool", &["maestro-request"]),
    ("maestro-brand", &["maestro-text", "maestro-json"]),
    ("maestro-pattern", &["maestro-text"]),
    ("maestro-markdown", &["maestro-text"]),
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
        &[
            "maestro-request",
            "maestro-pattern",
            "maestro-text",
            "maestro-listing",
        ],
    ),
    ("maestro-settings", &["maestro-lock", "maestro-json"]),
    ("maestro-test-conventions", &["maestro-markdown"]),
    ("maestro-test-process", &[]),
    ("maestro-test-directory", &[]),
    ("maestro-tooling", &[]),
    (
        "maestro-tui",
        &[
            "maestro-cancellation",
            "maestro-text",
            "maestro-process",
            "maestro-markdown",
            "maestro-listing",
        ],
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
            "maestro-listing",
        ],
    ),
    (
        "maestro-tui-crossterm",
        &["maestro-tui", "maestro-text", "maestro-process"],
    ),
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
            "maestro-json",
            "maestro-process",
            "maestro-listing",
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
            "maestro-protocol",
            "maestro-daemon",
            "maestro-markdown",
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
            "maestro-process",
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
    (
        "maestro-multiplexer",
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
            "maestro-multiplexer",
        ],
    ),
];

/// Internal development edges permitted beyond the production table.
pub(crate) const DEVELOPMENT: &[(&str, &str)] = &[
    ("maestro-detection", "maestro-text"),
    ("maestro-tui-crossterm", "maestro-brand"),
    ("maestro-extensions", "maestro-credentials"),
];

/// Foundation utility that native crates may use beyond their table row.
pub(crate) const UTILITY: &str = "maestro-path";

/// Dependency-free test-support crate reachable only through native development edges.
pub(crate) const TEST_PROCESS: &str = "maestro-test-process";

/// The only crates that may declare the test-process owner as a development dependency.
pub(crate) const TEST_PROCESS_CALLERS: &[&str] = &[
    "maestro-agent",
    "maestro-app",
    "maestro-catalog",
    "maestro-chat",
    "maestro-credentials",
    "maestro-daemon",
    "maestro-detection",
    "maestro-extensions",
    "maestro-ipc",
    "maestro-models",
    "maestro-packages",
    "maestro-process",
    "maestro-resources",
    "maestro-session",
    "maestro-settings",
    "maestro-test-conventions",
    "maestro-tooling",
    "maestro-tools",
    "maestro-tui",
    "maestro-tui-crossterm",
];

/// Native disposable-directory owner.
pub(crate) const TEST_DIRECTORY: &str = "maestro-test-directory";

/// Declared consumers of disposable native directory ownership.
pub(crate) const TEST_DIRECTORY_CALLERS: &[&str] = &[
    "maestro-app",
    "maestro-chat",
    "maestro-credentials",
    "maestro-extensions",
    "maestro-models",
    "maestro-packages",
    "maestro-resources",
    "maestro-test-conventions",
    "maestro-tooling",
];

/// Resolve one named support owner without granting a crate-name pattern.
pub(crate) fn test_support_callers(owner: &str) -> Option<&'static [&'static str]> {
    match owner {
        TEST_PROCESS => Some(TEST_PROCESS_CALLERS),
        TEST_DIRECTORY => Some(TEST_DIRECTORY_CALLERS),
        _ => None,
    }
}

/// Generated crate that unifies native dependency features and exposes no API.
pub(crate) const UNIFIER: &str = "maestro-dependencies";

/// Report whether a crate may declare the foundation utility as a production dependency.
pub(crate) fn uses_utility(name: &str) -> bool {
    name != UTILITY
        && name != UNIFIER
        && test_support_callers(name).is_none()
        && !matches!(
            name,
            "maestro-watch"
                | "maestro-lock"
                | "maestro-process"
                | "maestro-listing"
                | "maestro-pattern"
                | "maestro-markdown"
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
    if name == UNIFIER {
        return Some(Rule {
            class: "dedicated",
            dependencies: &[],
        });
    }
    let dependencies = POLICY.iter().find(|row| row.0 == name)?.1;
    Some(Rule {
        class: if matches!(
            name,
            "maestro"
                | "maestro-test-conventions"
                | "maestro-test-process"
                | "maestro-test-directory"
                | "maestro-test-terminal"
                | "maestro-tooling"
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
            | "maestro-multiplexer"
            | "maestro-chat"
            | "maestro-rpc"
            | "maestro-web"
            | "maestro-tui-crossterm"
            | "maestro-test-terminal"
    )
}

/// List required edges independently from the allowed dependency boundary.
pub(crate) fn required(name: &str) -> &'static [&'static str] {
    if name == "maestro" {
        &["maestro-app", "maestro-multiplexer"]
    } else if matches!(
        name,
        "maestro-cli" | "maestro-chat" | "maestro-rpc" | "maestro-web" | "maestro-multiplexer"
    ) {
        &["maestro-app"]
    } else if name == "maestro-daemon" {
        &["maestro-terminal"]
    } else if name == "maestro-tui-crossterm" {
        &["maestro-tui", "maestro-text"]
    } else if exact(name) {
        rule(name).map_or(&[], |rule| rule.dependencies)
    } else {
        &[]
    }
}
