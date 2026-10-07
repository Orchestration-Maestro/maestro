pub(crate) struct Rule {
    pub(crate) class: &'static str,
    pub(crate) dependencies: &'static [&'static str],
}
pub(crate) const POLICY: &[(&str, &[&str])] = &[
    ("maestro-extensions-wasm", &[]),
    ("maestro-models", &[]),
    ("maestro-resources", &[]),
    ("maestro-settings", &[]),
    ("maestro-storage", &[]),
    ("maestro-test-conventions", &[]),
    ("maestro-tooling", &[]),
    ("maestro-tui", &[]),
    ("maestro-agent", &["maestro-models"]),
    ("maestro-credentials", &["maestro-models"]),
    (
        "maestro-packages",
        &["maestro-settings", "maestro-resources"],
    ),
    ("maestro-test-terminal", &["maestro-tui"]),
    ("maestro-theme", &["maestro-tui"]),
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
