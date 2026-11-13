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

/// The frontends and the composition root, whose rows of the table bound the lower crates they
/// name and whose paths below the application the reach check resolves.
pub(crate) const FRONTENDS: &[&str] = &[
    "maestro",
    "maestro-chat",
    "maestro-cli",
    "maestro-multiplexer",
    "maestro-rpc",
    "maestro-web",
];

/// Internal development edges permitted beyond the production table on any target.
pub(crate) const DEVELOPMENT: &[(&str, &str)] = &[
    ("maestro-chat", "maestro-credentials"),
    ("maestro-tui-crossterm", "maestro-brand"),
];

/// Permitted development edges that only a native, non-optional declaration may use.
pub(crate) const NATIVE_DEVELOPMENT: &[(&str, &str)] = &[
    ("maestro-catalog", "maestro-providers"),
    ("maestro-cli", "maestro-providers"),
    ("maestro-extensions-wasmtime", "maestro-catalog"),
    ("maestro-extensions-wasmtime", "maestro-credentials"),
    ("maestro-extensions-wasmtime", "maestro-models"),
    ("maestro-extensions-wasmtime", "maestro-providers"),
    ("maestro-extensions-wasmtime", "maestro-session"),
    ("maestro-extensions-wasmtime", "maestro-theme"),
    ("maestro-extensions-wasmtime", "maestro-tui"),
    ("maestro-extensions", "maestro-credentials"),
    ("maestro-rpc", "maestro-theme"),
    ("maestro-rpc", "maestro-tui-crossterm"),
    ("maestro-rpc", "maestro-export"),
    ("maestro-rpc", "maestro-extensions"),
    ("maestro-rpc", "maestro-models"),
    ("maestro-rpc", "maestro-session"),
    ("maestro-rpc", "maestro-tools"),
    ("maestro-extensions-wasm", "maestro-models"),
    ("maestro-extensions-wasm", "maestro-extensions-wasmtime"),
    ("maestro-extensions-wasm", "maestro-listeners"),
    ("maestro-detection", "maestro-text"),
    ("maestro-cli", "maestro-listeners"),
    ("maestro-chat", "maestro-models"),
    ("maestro-chat", "maestro-tui-crossterm"),
    ("maestro", "maestro-lock"),
    ("maestro", "maestro-catalog"),
    ("maestro", "maestro-credentials"),
    ("maestro", "maestro-json"),
    ("maestro", "maestro-session"),
    ("maestro-theme", "maestro-realpath"),
];

/// Whether the policy permits this internal development edge between the two crates.
pub(crate) fn permitted_development(from: &str, to: &str) -> bool {
    DEVELOPMENT.contains(&(from, to)) || NATIVE_DEVELOPMENT.contains(&(from, to))
}

/// Internal build edges permitted beyond the production table.
pub(crate) const BUILD: &[(&str, &str)] = &[];

/// Whether the policy permits this internal build edge between the two crates.
pub(crate) fn permitted_build(from: &str, to: &str) -> bool {
    BUILD.contains(&(from, to))
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
    "maestro-extensions-wasm",
    "maestro-extensions-wasmtime",
    "maestro-http",
    "maestro-lock",
    "maestro-models",
    "maestro-packages",
    "maestro-process",
    "maestro-providers",
    "maestro-resources",
    "maestro-rpc",
    "maestro-session",
    "maestro-session-reports",
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
    "maestro-http",
    "maestro-listing",
    "maestro-lock",
    "maestro-models",
    "maestro-packages",
    "maestro-process",
    "maestro-resources",
    "maestro-rpc",
    "maestro-session",
    "maestro-session-reports",
    "maestro-settings",
    "maestro-terminal-emulator",
    "maestro-test-browser",
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

/// Native owner of scoped tracing subscribers that see every callsite.
pub(crate) const TEST_TRACING: &str = "maestro-test-tracing";

/// Declared consumers of scoped test tracing.
pub(crate) const TEST_TRACING_CALLERS: &[&str] = &[
    "maestro-daemon",
    "maestro-http",
    "maestro-process",
    "maestro-terminal-emulator",
    "maestro-workspace",
];

/// Portable owner of cross-package test fixture operations.
pub(crate) const TEST_FIXTURES: &str = "maestro-test-fixtures";

/// Declared consumers of portable fixture support.
pub(crate) const TEST_FIXTURE_CALLERS: &[&str] = &[
    "maestro-models",
    "maestro-request",
    "maestro-catalog",
    "maestro-chat",
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
        _ => None,
    }
}

/// Generated crate that unifies native dependency features and exposes no API.
pub(crate) const UNIFIER: &str = "maestro-dependencies";

/// Owner of mutual exclusion over files and shared in-memory state.
pub(crate) const LOCK: &str = "maestro-lock";

/// Report whether `from` may reach `to` as the checked child launcher's only edge to
/// the directory owner: the launcher declares that owner normally, so its fixtures
/// read the platform directory it hands to its cleared children through that edge.
pub(crate) fn directory_observation(from: &str, to: &str) -> bool {
    from == TEST_PROCESS && to == TEST_DIRECTORY
}

/// Report whether a crate may declare the foundation utility as a production dependency.
pub(crate) fn uses_utility(name: &str) -> bool {
    !matches!(name, UTILITY | UNIFIER)
        && test_support_callers(name).is_none()
        && !matches!(
            name,
            "maestro-watch"
                | "maestro-process"
                | "maestro-listing"
                | "maestro-pattern"
                | "maestro-markdown"
                | "maestro-office"
                | "maestro-cancellation"
                | "maestro-request"
                | "maestro-extensions-wasm"
                | "maestro-text"
                | "maestro-json"
                | "maestro-entry"
                | "maestro-tool"
                | "maestro-package"
                | "maestro-brand"
        )
}

/// Look up a scoped crate's classification and permitted dependencies.
pub(crate) fn rule(name: &str) -> Option<Rule> {
    Some(Rule {
        class: if matches!(
            name,
            UNIFIER
                | "maestro"
                | "maestro-test-browser"
                | "maestro-test-conventions"
                | "maestro-test-process"
                | "maestro-test-directory"
                | "maestro-test-fixtures"
                | "maestro-test-tracing"
                | "maestro-session-reports"
                | "maestro-tooling"
        ) {
            "dedicated"
        } else {
            "core"
        },
        dependencies: if name == UNIFIER {
            &[]
        } else {
            POLICY.iter().find(|row| row.0 == name)?.1
        },
    })
}

/// List required edges independently from the allowed dependency boundary.
pub(crate) fn required(name: &str) -> &'static [&'static str] {
    match name {
        "maestro" | "maestro-cli" | "maestro-chat" | "maestro-rpc" | "maestro-web" => {
            &["maestro-app"]
        }
        "maestro-daemon" => &["maestro-terminal-emulator"],
        "maestro-tui-crossterm" => &["maestro-tui", "maestro-text"],
        _ => &[],
    }
}
