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
    ("maestro-cancellation", &["maestro-listeners"]),
    ("maestro-request", &["maestro-json"]),
    ("maestro-watch", &[]),
    ("maestro-lock", &["maestro-path"]),
    ("maestro-process", &[]),
    ("maestro-listing", &[]),
    ("maestro-listeners", &[]),
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
            "maestro-listeners",
        ],
    ),
    (
        "maestro-resources",
        &[
            "maestro-request",
            "maestro-pattern",
            "maestro-text",
            "maestro-json",
            "maestro-listing",
        ],
    ),
    (
        "maestro-settings",
        &["maestro-lock", "maestro-json", "maestro-resources"],
    ),
    ("maestro-test-conventions", &["maestro-markdown"]),
    ("maestro-test-process", &[]),
    ("maestro-test-directory", &[]),
    ("maestro-test-fixtures", &[]),
    ("maestro-test-tracing", &[]),
    ("maestro-tooling", &[]),
    (
        "maestro-tui",
        &[
            "maestro-cancellation",
            "maestro-text",
            "maestro-process",
            "maestro-markdown",
            "maestro-listing",
            "maestro-resources",
            "maestro-listeners",
        ],
    ),
    (
        "maestro-agent",
        &[
            "maestro-models",
            "maestro-text",
            "maestro-tool",
            "maestro-listeners",
            "maestro-json",
        ],
    ),
    (
        "maestro-credentials",
        &[
            "maestro-models",
            "maestro-request",
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
        &[
            "maestro-tui",
            "maestro-text",
            "maestro-process",
            "maestro-listeners",
        ],
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
            "maestro-listing",
            "maestro-resources",
            "maestro-lock",
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
            "maestro-resources",
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
            "maestro-listeners",
        ],
    ),
    (
        "maestro-app",
        &[
            "maestro-workspace",
            "maestro-protocol",
            "maestro-daemon",
            "maestro-terminal",
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
            "maestro-listing",
            "maestro-listeners",
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
            "maestro-protocol",
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
            "maestro-theme",
            "maestro-tui",
            "maestro-tui-crossterm",
            "maestro-process",
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

/// Internal development edges permitted beyond the production table on any target.
pub(crate) const DEVELOPMENT: &[(&str, &str)] = &[("maestro-tui-crossterm", "maestro-brand")];

/// Permitted development edges that only a native, non-optional declaration may use.
pub(crate) const NATIVE_DEVELOPMENT: &[(&str, &str)] = &[
    ("maestro-extensions-wasmtime", "maestro-catalog"),
    ("maestro-extensions-wasmtime", "maestro-credentials"),
    ("maestro-extensions-wasmtime", "maestro-models"),
    ("maestro-extensions-wasmtime", "maestro-session"),
    ("maestro-extensions-wasmtime", "maestro-theme"),
    ("maestro-extensions-wasmtime", "maestro-tui"),
    ("maestro-extensions", "maestro-credentials"),
    ("maestro-rpc", "maestro-theme"),
    ("maestro-rpc", "maestro-tui-crossterm"),
    ("maestro-extensions-wasm", "maestro-models"),
    ("maestro-detection", "maestro-text"),
    ("maestro-cli", "maestro-listeners"),
    ("maestro-chat", "maestro-models"),
    ("maestro-chat", "maestro-tui-crossterm"),
];

/// Whether the policy permits this internal development edge between the two crates.
pub(crate) fn permitted_development(from: &str, to: &str) -> bool {
    [DEVELOPMENT, NATIVE_DEVELOPMENT]
        .iter()
        .any(|edges| edges.contains(&(from, to)))
}

/// Foundation utility that native crates may use beyond their table row.
pub(crate) const UTILITY: &str = "maestro-path";

/// Dependency-free test-support crate reachable only through native development edges.
pub(crate) const TEST_PROCESS: &str = "maestro-test-process";

/// The only crates that may declare the test-process owner as a development dependency.
pub(crate) const TEST_PROCESS_CALLERS: &[&str] = &[
    "maestro",
    "maestro-agent",
    "maestro-app",
    "maestro-catalog",
    "maestro-chat",
    "maestro-cli",
    "maestro-credentials",
    "maestro-daemon",
    "maestro-detection",
    "maestro-extensions",
    "maestro-extensions-wasmtime",
    "maestro-ipc",
    "maestro-lock",
    "maestro-models",
    "maestro-packages",
    "maestro-process",
    "maestro-resources",
    "maestro-rpc",
    "maestro-session",
    "maestro-settings",
    "maestro-test-conventions",
    "maestro-theme",
    "maestro-tooling",
    "maestro-tools",
    "maestro-tui",
    "maestro-tui-crossterm",
];

/// Native disposable-directory owner.
pub(crate) const TEST_DIRECTORY: &str = "maestro-test-directory";

/// Declared consumers of disposable native directory ownership.
pub(crate) const TEST_DIRECTORY_CALLERS: &[&str] = &[
    "maestro",
    "maestro-app",
    "maestro-catalog",
    "maestro-chat",
    "maestro-cli",
    "maestro-credentials",
    "maestro-daemon",
    "maestro-detection",
    "maestro-export",
    "maestro-extensions",
    "maestro-extensions-wasmtime",
    "maestro-ipc",
    "maestro-listing",
    "maestro-lock",
    "maestro-models",
    "maestro-packages",
    "maestro-process",
    "maestro-resources",
    "maestro-rpc",
    "maestro-session",
    "maestro-settings",
    "maestro-terminal",
    "maestro-test-conventions",
    "maestro-test-process",
    "maestro-theme",
    "maestro-tooling",
    "maestro-tools",
    "maestro-tui",
    "maestro-tui-crossterm",
    "maestro-watch",
    "maestro-workspace",
];

/// Native owner of the recording terminal and manual runtime every scenario needs.
pub(crate) const TEST_TERMINAL: &str = "maestro-test-terminal";

/// Declared consumers of the shared terminal scenario effects.
pub(crate) const TEST_TERMINAL_CALLERS: &[&str] = &["maestro-cli"];

/// Native owner of scoped tracing subscribers that see every callsite.
pub(crate) const TEST_TRACING: &str = "maestro-test-tracing";

/// Declared consumers of scoped test tracing.
pub(crate) const TEST_TRACING_CALLERS: &[&str] = &[
    "maestro-daemon",
    "maestro-models",
    "maestro-process",
    "maestro-terminal",
    "maestro-workspace",
];

/// Portable owner of cross-package test fixture operations.
pub(crate) const TEST_FIXTURES: &str = "maestro-test-fixtures";

/// Declared consumers of portable fixture support.
pub(crate) const TEST_FIXTURE_CALLERS: &[&str] = &[
    "maestro-agent",
    "maestro-models",
    "maestro-request",
    "maestro-catalog",
    "maestro-chat",
    "maestro-credentials",
    "maestro-tools",
    "maestro-export",
];

/// Resolve one named support owner without granting a crate-name pattern.
pub(crate) fn test_support_callers(owner: &str) -> Option<&'static [&'static str]> {
    match owner {
        TEST_PROCESS => Some(TEST_PROCESS_CALLERS),
        TEST_FIXTURES => Some(TEST_FIXTURE_CALLERS),
        TEST_DIRECTORY => Some(TEST_DIRECTORY_CALLERS),
        TEST_TRACING => Some(TEST_TRACING_CALLERS),
        TEST_TERMINAL => Some(TEST_TERMINAL_CALLERS),
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
                | "maestro-process"
                | "maestro-listing"
                | "maestro-pattern"
                | "maestro-markdown"
                | "maestro-cancellation"
                | "maestro-request"
                | "maestro-extensions-wasm"
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
                | "maestro-test-fixtures"
                | "maestro-test-terminal"
                | "maestro-test-tracing"
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
    match name {
        "maestro" => &["maestro-app"],
        "maestro-cli" | "maestro-chat" | "maestro-rpc" | "maestro-web" | "maestro-multiplexer" => {
            &["maestro-app"]
        }
        "maestro-daemon" => &["maestro-terminal"],
        "maestro-tui-crossterm" => &["maestro-tui", "maestro-text"],
        name if exact(name) => rule(name).map_or(&[], |rule| rule.dependencies),
        _ => &[],
    }
}
