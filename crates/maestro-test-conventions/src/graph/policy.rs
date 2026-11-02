/// Crate dependency table.
#[path = "policy/table.rs"]
mod table;

pub(crate) use table::POLICY;

/// Classification and permitted direct dependencies for a scoped crate.
pub(crate) struct Rule {
    /// Required workspace classification for this crate.
    pub(crate) class: &'static str,
    /// Names of permitted direct internal dependencies.
    pub(crate) dependencies: &'static [&'static str],
}

/// Internal development edges permitted beyond the production table on any target.
pub(crate) const DEVELOPMENT: &[(&str, &str)] = &[
    ("maestro-tui-crossterm", "maestro-brand"),
    ("maestro-extensions-wasm", "maestro-listing"),
];

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
    ("maestro-extensions-wasm", "maestro-extensions-wasmtime"),
    ("maestro-detection", "maestro-text"),
    ("maestro-cli", "maestro-listeners"),
    ("maestro-chat", "maestro-models"),
    ("maestro-chat", "maestro-tui-crossterm"),
];

/// Whether the policy permits this internal development edge between the two crates.
pub(crate) fn permitted_development(from: &str, to: &str) -> bool {
    DEVELOPMENT.contains(&(from, to)) || NATIVE_DEVELOPMENT.contains(&(from, to))
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
    "maestro-extensions-wasm",
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
