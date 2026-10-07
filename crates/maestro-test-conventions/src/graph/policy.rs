const APP_DEPENDENCIES: &[&str] = &[
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
];

pub(super) struct Rule {
    pub(super) class: &'static str,
    pub(super) dependencies: &'static [&'static str],
}
fn dependencies(name: &str) -> Option<&'static [&'static str]> {
    let dependencies: &'static [&'static str] = match name {
        "maestro-extensions-wasm"
        | "maestro-models"
        | "maestro-resources"
        | "maestro-settings"
        | "maestro-storage"
        | "maestro-test-conventions"
        | "maestro-tooling"
        | "maestro-tui" => &[],
        "maestro-agent" | "maestro-credentials" => &["maestro-models"],
        "maestro-packages" => &["maestro-settings", "maestro-resources"],
        "maestro-test-terminal" | "maestro-theme" | "maestro-tui-crossterm" => &["maestro-tui"],
        "maestro-catalog" => &["maestro-models", "maestro-credentials"],
        "maestro-session" => &["maestro-models", "maestro-agent", "maestro-storage"],
        "maestro-tools" => &[
            "maestro-models",
            "maestro-agent",
            "maestro-tui",
            "maestro-theme",
        ],
        "maestro-export" => &[
            "maestro-session",
            "maestro-models",
            "maestro-tools",
            "maestro-theme",
            "maestro-tui",
        ],
        "maestro-extensions" => &[
            "maestro-models",
            "maestro-agent",
            "maestro-session",
            "maestro-catalog",
            "maestro-tools",
            "maestro-theme",
            "maestro-tui",
            "maestro-resources",
        ],
        "maestro-app" => APP_DEPENDENCIES,
        "maestro-extensions-wasmtime" => &["maestro-extensions"],
        "maestro-chat" | "maestro-cli" => &[
            "maestro-app",
            "maestro-tui",
            "maestro-tui-crossterm",
            "maestro-theme",
        ],
        "maestro-rpc" | "maestro-web" => &["maestro-app", "maestro-theme"],
        "maestro" => &[
            "maestro-app",
            "maestro-cli",
            "maestro-rpc",
            "maestro-chat",
            "maestro-web",
            "maestro-extensions-wasmtime",
        ],
        _ => return None,
    };
    Some(dependencies)
}

pub(super) fn rule(name: &str) -> Option<Rule> {
    let dependencies = dependencies(name)?;
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
